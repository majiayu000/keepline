/**
 * Offline 「你在这里」card generation for accuracy sampling.
 *
 * Reads Claude Code / Codex transcripts (read-only) into an isolated Keepline database and
 * writes the derived inbox snapshot. Never touches hooks or the default ~/.keepline.
 *
 *   KEEPLINE_HOME=/tmp/kl-sample bun scripts/inbox-card-sample.ts out.json [maxAgeDays]
 */
import { homedir } from 'os';
import { join, resolve } from 'path';
import { writeFileSync } from 'fs';

const home = process.env.KEEPLINE_HOME;
if (!home || resolve(home) === join(homedir(), '.keepline')) {
  console.error('Set KEEPLINE_HOME to an isolated directory (not ~/.keepline).');
  process.exit(2);
}
const out = process.argv[2];
const maxAgeDays = Number(process.argv[3] ?? 30);
if (!out || !Number.isFinite(maxAgeDays)) {
  console.error('usage: KEEPLINE_HOME=<dir> bun scripts/inbox-card-sample.ts <out.json> [maxAgeDays]');
  process.exit(2);
}

const { runMigrations } = await import('../src/db/migrations.js');
const { syncSessions } = await import('../src/services/session.service.js');
const { inboxSnapshot } = await import('../src/services/inbox.service.js');
const { closeDatabase } = await import('../src/infrastructure/database/sqlite.js');

runMigrations();
const started = Date.now();
const sync = await syncSessions({ maxAgeDays, includeSubAgents: true });
const snapshot = await inboxSnapshot();
writeFileSync(out, JSON.stringify({ sync, elapsedMs: Date.now() - started, ...snapshot }, null, 2));
const counts = snapshot.rows.reduce<Record<string, number>>((acc, row) => ({ ...acc, [row.group]: (acc[row.group] ?? 0) + 1 }), {});
console.log(JSON.stringify({ rows: snapshot.rows.length, counts, elapsedMs: Date.now() - started }));
closeDatabase();
