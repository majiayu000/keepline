import { describe,test,expect } from 'bun:test';
import { codexTurnStatus,parseOpenRollouts,readCodexMetadata,closeCodexMetadata } from '../../adapters/codex/liveness.js';
import { resolveAggregatedStatus } from '../../services/session.aggregator.js';
import { sampleFacts } from './helpers.js';
import { Database } from 'bun:sqlite';
import { mkdtempSync,rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
describe('Codex liveness',() => {
  test('batched open files identify desktop sessions by file, not cwd',() => {
    const paths = parseOpenRollouts('p42\nn/home/.codex/sessions/rollout-live.jsonl\np43\nn/tmp/other');
    expect(paths.get('/home/.codex/sessions/rollout-live.jsonl')).toBe(42); expect(paths.size).toBe(1);
  });
  test('turn state wins over timing',() => {
    const facts = sampleFacts(); const now = new Date();
    expect(codexTurnStatus(facts,true,now,900)).toBe('idle');
    const started = facts.slice(0,1); expect(codexTurnStatus(started,true,now,900)).toBe('running');
    expect(codexTurnStatus(started,false,new Date(now.getTime()-901000),900)).toBe('stalled');
    expect(codexTurnStatus([{ kind: 'turn',phase: 'aborted',turnId: 't',at: now.toISOString(),reason: 'interrupt' }],true,now,900)).toBe('interrupted');
  });
  test('dashboard aggregation preserves completed Codex turn without a process',() => {
    expect(resolveAggregatedStatus({ client: 'codex',statusReason: 'Codex turn idle',status: 'idle',statusSource: 'scan',lastActiveAt: new Date(0) },undefined)).toBe('idle');
  });
  test('missing optional databases are harmless',() => expect(readCodexMetadata('test','/nonexistent')).toEqual({}));
  test('desktop name is exposed separately from its original prompt title',() => {
    const root = mkdtempSync(join(tmpdir(),'keepline-thread-name-')),db = new Database(join(root,'state_5.sqlite'));
    try {
      db.exec('CREATE TABLE threads(id TEXT,title TEXT,name TEXT); CREATE TABLE thread_spawn_edges(parent_thread_id TEXT,child_thread_id TEXT)');
      db.query('INSERT INTO threads VALUES(?,?,?)').run('thread','Original long prompt','发版并冻结格式');
      expect(readCodexMetadata('thread',root)).toEqual({ title: 'Original long prompt',name: '发版并冻结格式' });
    } finally { closeCodexMetadata(); db.close(); rmSync(root,{ recursive: true }); }
  });
  test('reads the actual thread_goals schema and all actionable goal states',() => {
    const root = mkdtempSync(join(tmpdir(),'keepline-goals-'));
    const db = new Database(join(root,'goals_1.sqlite'));
    try {
      db.exec('CREATE TABLE thread_goals(thread_id TEXT PRIMARY KEY,status TEXT,updated_at_ms INTEGER)');
      for (const status of ['usage_limited','budget_limited','blocked','complete']) {
        db.query('INSERT OR REPLACE INTO thread_goals VALUES(?,?,?)').run('thread',status,Date.now());
        expect(readCodexMetadata('thread',root)).toEqual(status === 'complete' ? {} : { limited: status });
      }
    } finally { closeCodexMetadata(); db.close(); rmSync(root,{ recursive: true }); }
  });
});
