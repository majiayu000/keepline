import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger,sampleFacts } from './helpers.js';
import { judgeLedger } from '../../services/ledger/judge.js';
import { config } from '../../lib/config.js';
import { closeDatabase } from '../../infrastructure/database/sqlite.js';
import { getLedger,replaceLedgerItems } from '../../services/ledger/service.js';
import type { Ask } from '../../domain/ledger/types.js';
import { readFileSync } from 'fs';
import { join } from 'path';
import { getKeeplineHome } from '../../lib/paths.js';

describe('optional requirement recognition',() => {
  setupLedgerTest();
  const enable = () => config.set('ledger',{ ...config.get().ledger,judge: { ...config.get().ledger.judge,enabled: true } });
  test('separate scanner and HTTP processes claim a user message only once',async () => {
    const d = await seededLedger(); enable(); const path = join(getKeeplineHome(),'model-calls.txt');
    const script = `import {appendFileSync} from 'fs'; const {judgeLedger}=await import('./src/services/ledger/judge.ts'); await judgeLedger(process.env.KEEPLINE_DECOMPOSE_AGENT,JSON.parse(process.env.KEEPLINE_DECOMPOSE_ASKS),async()=>{appendFileSync(process.env.KEEPLINE_DECOMPOSE_COUNT_FILE,'call\\n');await Bun.sleep(100);return {items:[{title:'Widget tests'}]}});`;
    const children = Array.from({ length: 2 },() => Bun.spawn(['bun','-e',script],{ env: { ...process.env,KEEPLINE_DECOMPOSE_AGENT: d.agentSessionId,KEEPLINE_DECOMPOSE_ASKS: JSON.stringify(d.asks),KEEPLINE_DECOMPOSE_COUNT_FILE: path },stdout: 'ignore',stderr: 'pipe' }));
    for (const child of children) { const error = await new Response(child.stderr).text(); expect(await child.exited,error).toBe(0); }
    expect(readFileSync(path,'utf8')).toBe('call\n');
  });
  test('one decomposition per user message survives concurrency, tool updates and restarts',async () => {
    const d = await seededLedger(); enable(); let calls = 0;
    const backend = async () => { calls++; await Bun.sleep(5); return { items: [{ title: 'Run widget tests',anchors: { commands: ['bun test src/widget.test.ts','delete everything'] },status: 'done',evidenceIds: ['invented'] }] }; };
    const results = await Promise.all([judgeLedger(d.agentSessionId,d.asks,backend),judgeLedger(d.agentSessionId,d.asks,backend)]);
    expect(calls).toBe(1);
    const item = results[0]![0]; expect(item.source).toBe('model'); expect(item.status).toBe('todo'); expect(item.evidenceIds).toEqual([]);
    expect(item.anchors.commands).toEqual(['bun test src/widget.test.ts']);
    closeDatabase(); expect((await judgeLedger(d.agentSessionId,d.asks,backend))![0].id).toBe(item.id); expect(calls).toBe(1);
    d.trail.push({ ...d.trail[0],callId: 'new-tool' }); d.claims.push({ text: 'Everything done',evidenceIds: [] });
    await judgeLedger(d.agentSessionId,d.asks,backend); expect(calls).toBe(1);
    const added: Ask = { id: 'addition',text: '性能再好一点',authoredText: '性能再好一点',kind: 'addition',at: new Date().toISOString() };
    const next = await judgeLedger(d.agentSessionId,[...d.asks,added],async prompt => { calls++; expect(prompt).toContain(added.text); return { items: [{ title: 'Improve performance' }] }; });
    expect(calls).toBe(2); expect(next?.map(i => i.title)).toEqual(['Run widget tests','Improve performance']);
    await judgeLedger(d.agentSessionId,[...d.asks,added],backend); expect(calls).toBe(2);
  });
  test('empty recognition suppresses a candidate, while failures keep it without retry storms',async () => {
    const d = await seededLedger(); enable(); let calls = 0;
    expect(await judgeLedger(d.agentSessionId,d.asks,async () => { calls++; return { items: [] }; })).toEqual([]);
    await judgeLedger(d.agentSessionId,d.asks,async () => { calls++; throw new Error('must not call'); }); expect(calls).toBe(1);
    const added: Ask = { id: 'new-ask',text: '我想做一个任务板',authoredText: '我想做一个任务板',kind: 'addition',at: new Date().toISOString() };
    const backend = async () => { calls++; throw new Error('backend offline'); };
    expect((await judgeLedger(d.agentSessionId,[...d.asks,added],backend))?.[0]).toMatchObject({ title: added.text,source: 'fallback' });
    await judgeLedger(d.agentSessionId,[...d.asks,added],backend); expect(calls).toBe(2);
  });
  test('questions have no model call or progress; user edits and removals survive subsequent scans',async () => {
    const d = await seededLedger([{ kind: 'user_message',text: '这个库还有什么没做的吗',at: new Date().toISOString() }]);
    expect(d.asks[0].kind).toBe('question'); expect(d.progress.total).toBe(0); enable();
    let calls = 0; expect(await judgeLedger(d.agentSessionId,d.asks,async () => { calls++; return { items: [] }; })).toEqual([]); expect(calls).toBe(0);
    config.set('ledger',{ ...config.get().ledger,judge: { ...config.get().ledger.judge,enabled: false } });
    const ordinary = await seededLedger(sampleFacts(),'editable-session');
    await replaceLedgerItems(ordinary.sessionId,[]); expect((await getLedger(ordinary.sessionId))?.items).toHaveLength(0);
    const extra = { ...ordinary.items[0],id: undefined as unknown as string,title: '我补上的验收项',status: 'todo' as const,evidenceIds: [] };
    const edited = await replaceLedgerItems(ordinary.sessionId,[extra]); expect(edited?.items.map(i => i.title)).toEqual([extra.title]);
    closeDatabase(); expect((await getLedger(ordinary.sessionId))?.items.map(i => i.title)).toEqual([extra.title]);
  });
  test('integration matches evidence after decomposition and never sends tool output to the model',async () => {
    enable(); const original = globalThis.fetch; let calls = 0;
    globalThis.fetch = (async (_url,init) => {
      calls++; const input = JSON.parse(String(init?.body));
      expect(input.messages[0].content).toContain('Run `bun test src/widget.test.ts`');
      expect(input.messages[0].content).not.toContain('3 pass');
      return Response.json({ choices: [{ message: { content: JSON.stringify({ items: [{ title: 'Run widget tests',anchors: { commands: ['bun test src/widget.test.ts'] },status: 'done',evidenceIds: ['invented'] }] }) } }] });
    }) as typeof fetch;
    config.set('ledger',{ ...config.get().ledger,judge: { ...config.get().ledger.judge,backend: 'local' } });
    try {
      const d = await seededLedger(); expect(d.progress).toEqual({ done: 1,total: 1 }); expect(d.items[0].statusSource).toBe('rule'); expect(calls).toBe(1);
      await getLedger(d.sessionId); closeDatabase(); await getLedger(d.sessionId); expect(calls).toBe(1);
      await replaceLedgerItems(d.sessionId,[]); expect((await getLedger(d.sessionId))?.items).toHaveLength(0); expect(calls).toBe(1);
      config.set('ledger',{ ...config.get().ledger,judge: { ...config.get().ledger.judge,enabled: false } });
      expect((await getLedger(d.sessionId))?.items).toHaveLength(0);
      config.set('ledger',{ ...config.get().ledger,judge: { ...config.get().ledger.judge,enabled: true } });
      expect((await getLedger(d.sessionId))?.items).toHaveLength(0); expect(calls).toBe(1);
    } finally { globalThis.fetch = original; }
  });
});
