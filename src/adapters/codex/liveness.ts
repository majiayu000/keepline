import { execFile } from 'child_process';
import { Database } from 'bun:sqlite';
import { existsSync } from 'fs';
import { join } from 'path';
import { homedir } from 'os';
import type { TranscriptFact } from '../../domain/ledger/types.js';
import type { SessionStatus } from '../../domain/session/index.js';

export function parseOpenRollouts(output: string): Map<string, number> {
  const paths = new Map<string, number>(); let pid = 0;
  for (const line of output.split('\n')) {
    if (line.startsWith('p')) pid = Number(line.slice(1));
    if (line.startsWith('n') && /rollout-[^/]+\.jsonl$/.test(line)) paths.set(line.slice(1), pid);
  }
  return paths;
}
export function probeOpenRollouts(): Promise<Map<string, number>> {
  if (process.platform !== 'darwin' && process.platform !== 'linux') return Promise.resolve(new Map());
  return new Promise(resolve => execFile('lsof', ['-Fn', '-c', 'codex', '-c', 'Codex'], { timeout: 5000, maxBuffer: 8 * 1024 * 1024 }, (_error, stdout) => {
    // lsof exits 1 when no matching process is holding files.
    resolve(parseOpenRollouts(stdout || ''));
  }));
}
export function codexTurnStatus(facts: TranscriptFact[], live: boolean, lastWrite: Date, stalledAfterSeconds: number, now = Date.now()): SessionStatus | undefined {
  const turn = [...facts].reverse().find(f => f.kind === 'turn');
  if (!turn || turn.kind !== 'turn') return undefined;
  if (turn.phase === 'completed') return 'idle';
  if (turn.phase === 'aborted') return 'interrupted';
  return live ? 'running' : now - lastWrite.getTime() >= stalledAfterSeconds * 1000 ? 'stalled' : 'running';
}
const metadataDatabases = new Map<string,Database>();
export function closeCodexMetadata() { for (const db of metadataDatabases.values()) db.close(); metadataDatabases.clear(); }
/** Optional local metadata only; unsupported schemas leave the normal transcript result intact. */
export function readCodexMetadata(rawId: string, root = join(homedir(), '.codex')): { title?: string; name?: string; parentId?: string; limited?: string } {
  const result: { title?: string; name?: string; parentId?: string; limited?: string } = {};
  for (const [file, read] of [
    ['state_5.sqlite', (db: Database) => {
      const row = db.query('SELECT title,name FROM threads WHERE id = ?').get(rawId) as { title: string; name: string | null } | null;
      if (row) { result.title = row.title; if (row.name?.trim()) result.name = row.name; }
      const edge = db.query('SELECT parent_thread_id FROM thread_spawn_edges WHERE child_thread_id = ?').get(rawId) as { parent_thread_id: string } | null;
      if (edge) result.parentId = edge.parent_thread_id;
    }],
    ['goals_1.sqlite', (db: Database) => {
      const row = db.query('SELECT status FROM thread_goals WHERE thread_id = ? ORDER BY updated_at_ms DESC LIMIT 1').get(rawId) as { status: string } | null;
      if (row && ['usage_limited','budget_limited','blocked'].includes(row.status)) result.limited = row.status;
    }],
  ] as const) {
    const path = join(root, file); if (!existsSync(path)) continue;
    try {
      let db = metadataDatabases.get(path);
      if (!db) { db = new Database(path,{ readonly: true }); metadataDatabases.set(path,db); }
      read(db);
    } catch { /* Optional schema enrichment. */ }
  }
  return result;
}
