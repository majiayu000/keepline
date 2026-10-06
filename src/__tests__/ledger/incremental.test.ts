import { describe,test,expect } from 'bun:test';
import { mkdtempSync,writeFileSync,appendFileSync,rmSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { setupLedgerTest } from './helpers.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { clearLedgerCache,ingestLedger,getLedger,replaceLedgerItems } from '../../services/ledger/service.js';
import { clearLedgerFactCache,ledgerFactCacheStats } from '../../services/ledger/facts.js';
import { closeSessionSummaryCache } from '../../infrastructure/session-summary-cache.js';
import { config } from '../../lib/config.js';

describe('incremental ledger with isolated scan restarts',() => {
  setupLedgerTest();
  test('cached vague sessions discover previous requirements indexed later',async () => {
    const root = mkdtempSync(join(tmpdir(),'ledger-import-order-'));
    const now = Date.now(), vagueId = crypto.randomUUID(), previousId = crypto.randomUUID();
    const seed = async (id: string,text: string,at: number) => {
      const path = join(root,`${id}.jsonl`),timestamp = new Date(at).toISOString();
      writeFileSync(path,JSON.stringify({ type: 'user',uuid: id,timestamp,message: { content: text } })+'\n');
      const parsed = { sessionId: id,directory: root,sourcePath: path,lastActiveAt: new Date(at),messageCount: 1,toolCount: 0 };
      const session = sessionRepository.upsert({ ...parsed,client: 'claude',title: text,status: 'idle' });
      return (await ingestLedger(session,parsed))!;
    };
    try {
      expect((await seed(vagueId,'Fix it',now)).importSuggestions).toEqual([]);
      await seed(previousId,'Run bun test widget',now-60000);
      const reads = ledgerFactCacheStats().reads;
      clearLedgerCache(); closeSessionSummaryCache();
      expect((await getLedger(vagueId))?.importSuggestions?.map(s => s.sessionId)).toContain(previousId);
      expect(ledgerFactCacheStats().reads).toBe(reads);
    } finally { clearLedgerCache(); clearLedgerFactCache(); closeSessionSummaryCache(); rmSync(root,{ recursive: true }); }
  });
  test('unchanged files skip fact loads and writes; edits, status, config and appends invalidate',async () => {
    const root = mkdtempSync(join(tmpdir(),'ledger-incremental-')),path = join(root,'session.jsonl');
    const id = crypto.randomUUID(),timestamp = new Date().toISOString();
    const records = [
      { type: 'user',uuid: 't',timestamp,message: { content: 'Run `bun test widget`' } },
      { type: 'assistant',timestamp,message: { content: [{ type: 'tool_use',id: 'check',name: 'Bash',input: { command: 'bun test widget' } }] } },
      { type: 'user',timestamp,message: { content: [{ type: 'tool_result',tool_use_id: 'check',content: '14 passed; 0 failed',is_error: false }] } },
      { type: 'assistant',timestamp,message: { content: 'Completed tests',stop_reason: 'end_turn' } },
    ];
    writeFileSync(path,records.map(r => JSON.stringify(r)).join('\n')+'\n');
    const parsed = { sessionId: id,directory: root,sourcePath: path,lastActiveAt: new Date(timestamp),messageCount: 2,toolCount: 1 };
    try {
      let session = sessionRepository.upsert({ ...parsed,client: 'claude',title: 'Widget',status: 'idle' });
      const first = (await ingestLedger(session,parsed))!; expect(first.progress.done).toBe(0);
      const reads = ledgerFactCacheStats().reads;
      const changes = () => (getDatabase().query('SELECT total_changes() AS n').get() as { n: number }).n;
      const before = changes(); clearLedgerCache(); clearLedgerFactCache(); closeSessionSummaryCache();
      expect(await ingestLedger(session,parsed)).toBeNull();
      expect(ledgerFactCacheStats().reads).toBe(reads); expect(changes()).toBe(before);
      expect((await getLedger(id))?.items[0].source).toBe('fallback');
      const edited = await replaceLedgerItems(id,[{ ...first.items[0],title: 'Run `bun test widget`' }]);
      expect(edited?.progress.done).toBe(1);
      clearLedgerCache(); closeSessionSummaryCache();
      expect((await getLedger(id))?.progress.done).toBe(1);
      session = sessionRepository.upsert({ sessionId: id,status: 'needs_input',statusSource: 'hook' });
      const statusReads = ledgerFactCacheStats().diskHits;
      expect(await ingestLedger(session,parsed)).toBeNull();
      expect(ledgerFactCacheStats().diskHits).toBe(statusReads);
      expect((await getLedger(id))?.state).toBe('needs_input');
      config.set('ledger',{ ...config.get().ledger,deviation: 'off' });
      expect(await ingestLedger(session,parsed)).not.toBeNull();
      appendFileSync(path,JSON.stringify({ type: 'user',uuid: 'injected',timestamp,message: { content: '<task-notification>Completed background work</task-notification>' } })+'\n'+JSON.stringify({ type: 'user',uuid: 't2',timestamp,message: { content: 'Also fix widget.ts' } })+'\n');
      const appended = await ingestLedger(session,parsed);
      expect(appended?.asks.map(a => a.text)).toEqual(['Run `bun test widget`','Also fix widget.ts']);
      expect(ledgerFactCacheStats().reads).toBe(reads+1);
    } finally { clearLedgerCache(); clearLedgerFactCache(); closeSessionSummaryCache(); rmSync(root,{ recursive: true }); }
  });
});
