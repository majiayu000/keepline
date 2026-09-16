import { Database } from 'bun:sqlite';
import { statSync } from 'fs';
import { join } from 'path';
import type { ParsedSessionData } from '../domain/session/index.js';
import { ensureKeeplineDataHome } from '../lib/paths.js';

// Increment when parser semantics change. This is derived data, never task/session truth.
const CACHE_VERSION = 1;
let database: Database | undefined;
const stats = { hits: 0, misses: 0, writes: 0 };

export class SessionSummaryCacheError extends Error {}

function cacheOperation<T>(operation: () => T): T {
  try { return operation(); }
  catch (error) {
    throw new SessionSummaryCacheError(`Session summary cache failed: ${error instanceof Error ? error.message : String(error)}`, { cause: error });
  }
}

function cacheDatabase(): Database {
  if (!database) {
    database = new Database(join(ensureKeeplineDataHome(), `session-summaries-v${CACHE_VERSION}.sqlite`));
    database.exec(`
      PRAGMA journal_mode = WAL;
      PRAGMA synchronous = NORMAL;
      PRAGMA busy_timeout = 5000;
      CREATE TABLE IF NOT EXISTS summaries (
        cache_key TEXT PRIMARY KEY,
        fingerprint TEXT NOT NULL,
        data TEXT NOT NULL
      );
    `);
  }
  return database;
}

function fingerprint(path: string): string {
  const stat = statSync(path);
  return JSON.stringify([stat.dev, stat.ino, stat.size, stat.mtimeMs, stat.ctimeMs]);
}

/** Cache transcript facts only; process matching and completion decisions still run each scan. */
export async function cachedSessionSummary<T extends ParsedSessionData>(
  runtime: 'claude' | 'codex',
  path: string,
  parse: () => Promise<T | null>
): Promise<T | null> {
  const before = fingerprint(path);
  const key = `${runtime}:${path}`;
  const db = cacheOperation(cacheDatabase);
  const row = cacheOperation(() => db.query('SELECT data FROM summaries WHERE cache_key = ? AND fingerprint = ?')
    .get(key, before) as { data: string } | null);
  if (row) {
    stats.hits++;
    const parsed = cacheOperation(() => JSON.parse(row.data) as T | null);
    if (parsed) {
      parsed.lastActiveAt = new Date(parsed.lastActiveAt);
      if (parsed.startedAt) parsed.startedAt = new Date(parsed.startedAt);
    }
    return parsed;
  }
  stats.misses++;
  const parsed = await parse();
  // A live transcript may grow during parsing. Do not bless that partial read as current.
  if (fingerprint(path) === before) {
    cacheOperation(() => db.query(`INSERT INTO summaries (cache_key, fingerprint, data) VALUES (?, ?, ?)
      ON CONFLICT(cache_key) DO UPDATE SET fingerprint = excluded.fingerprint, data = excluded.data`)
      .run(key, before, JSON.stringify(parsed)));
    stats.writes++;
  }
  return parsed;
}

export function sessionSummaryCacheStats(): Readonly<typeof stats> {
  return { ...stats };
}

export function closeSessionSummaryCache(): void {
  database?.close();
  database = undefined;
}
