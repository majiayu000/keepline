import { serialize, deserialize } from 'node:v8';
import { createHash } from 'node:crypto';
import type { JsonlCheckpoint, JsonlCursorOptions } from '../adapters/jsonl-cursor.js';
import { TranscriptFacts, type TranscriptFactsState } from '../adapters/transcript-facts.js';
import { config } from '../lib/config.js';
import { Database } from 'bun:sqlite';
import { statSync, openSync, readSync, closeSync } from 'fs';
import { join } from 'path';
import type { ParsedSessionData } from '../domain/session/index.js';
import { ensureKeeplineDataHome } from '../lib/paths.js';

// Increment when parser semantics change. This is derived data, never task/session truth.
const CACHE_VERSION = 2;
// Keep computed fingerprints and persistent cleanup on the same semantic version.
export const LEDGER_COMPUTATION_VERSION = 35;
let database: Database | undefined;
let ledgerWindow: string | undefined;
let computationWindow: string | undefined;
const stats = { hits: 0, misses: 0, writes: 0 };
const incrementalStats = { resumed: 0, bytes: 0 };
export function sessionIncrementalStats() { return { ...incrementalStats }; }
interface ParserCursor {
  window: string;
  factWindow: string;
  dev: number;
  ino: number;
  size: number;
  guard: string;
  checkpoint: JsonlCheckpoint;
  facts?: TranscriptFactsState;
  scopeKnown: boolean;
}
function prefixGuard(path: string, offset: number): string {
  const fd = openSync(path,'r');
  try {
    const size = Math.min(offset,4096), first = Buffer.alloc(size), last = Buffer.alloc(size);
    const firstRead = readSync(fd,first,0,size,0), lastRead = readSync(fd,last,0,size,offset-size);
    return createHash('sha256').update(first.subarray(0,firstRead)).update(last.subarray(0,lastRead)).digest('hex');
  } finally { closeSync(fd); }
}

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
  parse: (onRecord?: (entry: unknown) => void, cursor?: JsonlCursorOptions) => Promise<T | null>
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
      parsed.sourcePath = path;
      parsed.lastActiveAt = new Date(parsed.lastActiveAt);
      if (parsed.startedAt) parsed.startedAt = new Date(parsed.startedAt);
    }
    return parsed;
  }
  stats.misses++;
  const cfg = config.get().ledger, info = statSync(path);
  const factKey = ledgerFactFingerprint(info);
  // Midnight changes the fact retention window, not the position in an append-only file.
  const window = JSON.stringify(['cursor-3',cfg.retentionDays,cfg.enabled,cfg.exclude]);
  const saved = cacheOperation(() => db.query('SELECT data FROM summaries WHERE cache_key=?').get(`cursor-v1:${key}`) as { data: Uint8Array } | null);
  let previous = saved ? cacheOperation(() => deserialize(saved.data) as ParserCursor) : undefined;
  if (previous && (previous.window !== window || previous.dev !== info.dev || previous.ino !== info.ino ||
      info.size <= previous.size || !previous.checkpoint.offset ||
      prefixGuard(path,previous.checkpoint.offset) !== previous.guard)) previous = undefined;
  // Rebuild when expiration could change turn context or unknown-record accounting.
  if (previous?.facts && (previous.facts.facts.some(f => Date.parse(f.at) < factKey.since) ||
      previous.factWindow !== factKey.window && previous.facts.unknownRecords > 0)) previous = undefined;
  if (previous) incrementalStats.resumed++;
  incrementalStats.bytes += info.size - (previous?.checkpoint.offset ?? 0);
  let facts = previous ? previous.facts ? new TranscriptFacts(runtime,factKey.since,previous.facts) : undefined
    : cfg.enabled && info.mtimeMs >= factKey.since && !cfg.exclude.runtimes.includes(runtime === 'codex' ? 'codex' : 'claude-code') ? new TranscriptFacts(runtime,factKey.since) : undefined;
  let observed = !!previous?.facts, scopeKnown = previous?.scopeKnown ?? false;
  let checkpointData: Buffer | undefined;
  const parsed = await parse(facts ? entry => {
    observed = true;
    const row = entry as { cwd?: string; payload?: { cwd?: string } } | null;
    const cwd = row?.cwd ?? row?.payload?.cwd;
    if (!scopeKnown && typeof cwd === 'string') {
      scopeKnown = true;
      if (cfg.exclude.projects.some(p => cwd === p || cwd.startsWith(`${p}/`))) facts = undefined;
    }
    if (entry) facts?.add(entry); else if (facts) facts.unknownRecords++;
  } : undefined, {
    resume: previous?.checkpoint,end: info.size,
    onCheckpoint: checkpoint => {
      // Serialize before processing a partial tail: later records may mutate prior tool calls.
      checkpointData = serialize({ window,factWindow: factKey.window,dev: info.dev,ino: info.ino,size: info.size,
        guard: prefixGuard(path,checkpoint.offset),checkpoint,facts: facts?.snapshot(),scopeKnown } satisfies ParserCursor);
    },
  });
  if (checkpointData) {
    const after = statSync(path);
    if (after.dev === info.dev && after.ino === info.ino && after.size >= info.size)
      cacheOperation(() => db.query('INSERT INTO summaries VALUES(?,?,?) ON CONFLICT(cache_key) DO UPDATE SET fingerprint=excluded.fingerprint,data=excluded.data')
        .run(`cursor-v1:${key}`,window,checkpointData!));
  }
  // A live transcript may grow during parsing. Do not bless that partial read as current.
  if (fingerprint(path) === before) {
    cacheOperation(() => db.query(`INSERT INTO summaries (cache_key, fingerprint, data) VALUES (?, ?, ?)
      ON CONFLICT(cache_key) DO UPDATE SET fingerprint = excluded.fingerprint, data = excluded.data`)
      .run(key, before, JSON.stringify(parsed ? { ...parsed, transcriptFacts: undefined } : null)));
    if (parsed && observed && facts) writeCachedLedgerFacts(`${runtime}:${path}`,factKey.fingerprint,{ fingerprint: factKey.fingerprint,facts: facts.facts,unknownRecords: facts.unknownRecords });
    stats.writes++;
  }
  facts = undefined;
  // Bun's native JSON/stream allocations are not reflected in JS heap pressure.
  // Reclaim large-file parse temporaries before scanning another transcript.
  if (statSync(path).size >= 8 * 1024 * 1024) Bun.gc(true);
  return parsed ? { ...parsed,transcriptFacts: undefined } : null;
}

export function sessionSummaryCacheStats(): Readonly<typeof stats> {
  return { ...stats };
}

export function closeSessionSummaryCache(): void {
  database?.close();
  database = undefined;
  ledgerWindow = undefined;
  computationWindow = undefined;
}

/** Ledger facts use the same derived-data disk cache, never the transcript summary arrays. */
export function readCachedLedgerFacts<T>(key: string, fingerprint: string, window: string): T | undefined {
  const db = cacheOperation(cacheDatabase);
  // Expired windows must not retain yesterday's transcript-derived data indefinitely.
  if (ledgerWindow !== window) {
    cacheOperation(() => db.query("DELETE FROM summaries WHERE cache_key LIKE 'ledger:%' AND fingerprint NOT LIKE ?").run(`${window}:%`));
    ledgerWindow = window;
  }
  const row = cacheOperation(() => db.query('SELECT data FROM summaries WHERE cache_key=? AND fingerprint=?').get(`ledger:${key}`,fingerprint) as { data: string } | null);
  return row ? cacheOperation(() => JSON.parse(row.data) as T) : undefined;
}
export function writeCachedLedgerFacts(key: string, fingerprint: string, value: unknown): void {
  cacheOperation(() => cacheDatabase().query(`INSERT INTO summaries VALUES(?,?,?) ON CONFLICT(cache_key) DO UPDATE SET fingerprint=excluded.fingerprint,data=excluded.data`).run(`ledger:${key}`,fingerprint,JSON.stringify(value)));
}

/** Computed ledger rows are derived data shared by the isolated scanner and HTTP process. */
export function readLedgerComputation<T>(key: string, fingerprint?: string): T | undefined {
  const day = new Date().toISOString().slice(0,10);
  const window = `ledger-${LEDGER_COMPUTATION_VERSION}:${day}`;
  if (computationWindow !== window) {
    cacheOperation(() => cacheDatabase().query("DELETE FROM summaries WHERE cache_key LIKE 'ledger-computed:%' AND fingerprint NOT LIKE ?").run(`ledger-${LEDGER_COMPUTATION_VERSION}:%:${day}:%`));
    computationWindow = window;
  }
  const row = cacheOperation(() => cacheDatabase().query(`SELECT data FROM summaries WHERE cache_key=? ${fingerprint ? 'AND fingerprint=?' : ''}`)
    .get(`ledger-computed:${key}`,...(fingerprint ? [fingerprint] : [])) as { data: string } | null);
  return row ? cacheOperation(() => JSON.parse(row.data) as T) : undefined;
}
export function writeLedgerComputation(key: string, fingerprint: string, value: unknown): void {
  cacheOperation(() => cacheDatabase().query(`INSERT INTO summaries VALUES(?,?,?) ON CONFLICT(cache_key) DO UPDATE SET fingerprint=excluded.fingerprint,data=excluded.data`)
    .run(`ledger-computed:${key}`,fingerprint,JSON.stringify(value)));
}

export function ledgerFactFingerprint(info: { mtimeMs: number; ctimeMs: number; size: number },now = Date.now()) {
  const days = config.get().ledger.retentionDays;
  const window = `facts-25-${days}-${new Date(now).toISOString().slice(0,10)}`;
  return { window,since: now-days*86400000,fingerprint: `${window}:${info.mtimeMs}:${info.ctimeMs}:${info.size}` };
}
