import { afterEach, expect, test, spyOn } from 'bun:test';
import { appendFileSync, mkdtempSync, renameSync, rmSync, writeFileSync, utimesSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { config } from '../lib/config.js';
import { cachedSessionSummary, closeSessionSummaryCache, sessionIncrementalStats } from '../infrastructure/session-summary-cache.js';
import { parseCodexSessionFile } from '../adapters/codex/parser.js';
import { parseSessionFile } from '../adapters/claude/parser/jsonl.js';
import type { JsonlCursorOptions } from '../adapters/jsonl-cursor.js';
import { readTranscriptFacts, clearLedgerFactCache } from '../services/ledger/facts.js';
import { TranscriptFacts } from '../adapters/transcript-facts.js';
import { decompose, matchLedger } from '../domain/ledger/matcher.js';
import { DEFAULT_LEDGER_CONFIG } from '../domain/ledger/types.js';

const roots: string[] = [];
const original = structuredClone(config.get().ledger);
afterEach(() => { closeSessionSummaryCache(); clearLedgerFactCache(); config.set('ledger',original); for (const dir of roots.splice(0)) rmSync(dir,{ recursive: true,force: true }); });

test('restart preserves asynchronous session receipts and mutation ordering until a fresh check completes', async () => {
  const root = mkdtempSync(join(tmpdir(), 'keepline-async-restart-')); roots.push(root);
  const path = join(root, 'session.jsonl'), timestamp = new Date().toISOString();
  config.set('ledger', { ...original, enabled: true, exclude: { projects: [], runtimes: [] } });
  const call = (id: string, name: string, input: unknown) => ({ type: 'response_item', timestamp, payload: { type: 'function_call', call_id: id, name, arguments: JSON.stringify(input) } });
  const result = (id: string, output: unknown) => ({ type: 'response_item', timestamp, payload: { type: 'function_call_output', call_id: id, output: JSON.stringify(output) } });
  const records = [
    { type: 'session_meta', timestamp, payload: { id: 'async-restart', cwd: root } },
    call('writer', 'exec_command', { cmd: 'sed -i.bak s/old/new/ widget.ts' }),
    result('writer', { session_id: 91, output: 'still running' }),
    call('during', 'exec_command', { cmd: 'bun test widget' }),
    result('during', { exit_code: 0, output: '1 pass' }),
  ];
  writeFileSync(path, records.map(record => JSON.stringify(record)).join('\n') + '\n');
  const parse = (onRecord?: (entry: unknown) => void, cursor?: JsonlCursorOptions) => parseCodexSessionFile(path, { includeToolCalls: false, onRecord, ...cursor });
  await cachedSessionSummary('codex', path, parse); closeSessionSummaryCache();
  appendFileSync(path, [call('poll', 'write_stdin', { session_id: 91, chars: '' }), result('poll', { exit_code: 0, output: '' })].map(record => JSON.stringify(record)).join('\n') + '\n');
  await cachedSessionSummary('codex', path, parse);
  const resumed = await readTranscriptFacts(path, 'codex');
  const fresh = new TranscriptFacts('codex'); await parse(entry => fresh.add(entry));
  expect(resumed.facts).toEqual(fresh.facts);
  const writer = resumed.facts.find(fact => fact.kind === 'tool' && fact.callId === 'writer');
  const during = resumed.facts.find(fact => fact.kind === 'tool' && fact.callId === 'during');
  expect(writer).toMatchObject({ kind: 'tool', completed: true, exitCode: 0 });
  if (writer?.kind === 'tool' && during?.kind === 'tool') expect(writer.completedOrder!).toBeGreaterThan(during.completedOrder!);
  const items = decompose([], 'async-restart', [{ id: 'check', text: 'Run `bun test widget`' }]);
  expect(matchLedger(resumed.facts, items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(0);
  closeSessionSummaryCache(); clearLedgerFactCache();
  appendFileSync(path, [call('after', 'exec_command', { cmd: 'bun test widget' }), result('after', { exit_code: 0, output: '1 pass' })].map(record => JSON.stringify(record)).join('\n') + '\n');
  await cachedSessionSummary('codex', path, parse);
  expect(matchLedger((await readTranscriptFacts(path, 'codex')).facts, items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(1);
});

for (const runtime of ['codex','claude'] as const) {
  test(`${runtime}: next-day append reuses the persisted position and writes fresh retention-window facts`,async () => {
    const root = mkdtempSync(join(tmpdir(),'keepline-next-day-')); roots.push(root);
    const path = join(root,'session.jsonl'), today = Date.now();
    config.set('ledger',{ ...original,enabled: true,retentionDays: 30,exclude: { projects: [],runtimes: [] } });
    const message = (role: 'user' | 'assistant',text: string,at: number) => runtime === 'codex'
      ? { type: 'response_item',timestamp: new Date(at).toISOString(),payload: { type: 'message',role,content: [{ type: role === 'user' ? 'input_text' : 'output_text',text }] } }
      : { type: role,sessionId: 'next-day',cwd: root,uuid: role,timestamp: new Date(at).toISOString(),message: { role,content: text } };
    const header = runtime === 'codex' ? [{ type: 'session_meta',timestamp: new Date(today).toISOString(),payload: { id: 'next-day',cwd: root } }] : [];
    writeFileSync(path,[...header,message('user','做一个任务板',today),{ type: runtime === 'codex' ? 'world_state' : 'file-history-snapshot',payload: 'x'.repeat(1024*1024) }].map(r => JSON.stringify(r)).join('\n')+'\n');
    const parse = (onRecord?: (entry: unknown) => void,cursor?: JsonlCursorOptions) => runtime === 'codex'
      ? parseCodexSessionFile(path,{ includeToolCalls: false,onRecord,...cursor })
      : parseSessionFile(path,{ includeToolCalls: false,onRecord,...cursor });
    await cachedSessionSummary(runtime,path,parse);closeSessionSummaryCache();
    const clock = spyOn(Date,'now').mockReturnValue(today+86400000);
    try {
      appendFileSync(path,JSON.stringify(message('assistant','第二天继续完成',today+86400000))+'\n');
      const before = sessionIncrementalStats();
      expect(await cachedSessionSummary(runtime,path,parse)).toEqual(await parse());
      const after = sessionIncrementalStats();
      expect(after.resumed-before.resumed).toBe(1);
      expect(after.bytes-before.bytes).toBeLessThan(1000);
      const facts = await readTranscriptFacts(path,runtime);
      expect(facts.facts.some(f => f.kind === 'user_message' && f.text === '做一个任务板')).toBe(true);
      expect(facts.facts.some(f => f.kind === 'agent_message' && f.text === '第二天继续完成')).toBe(true);
      closeSessionSummaryCache();
      clock.mockReturnValue(today+31*86400000);
      appendFileSync(path,JSON.stringify(message('assistant','保留期后追加',today+31*86400000))+'\n');
      utimesSync(path,new Date(Date.now()),new Date(Date.now()));
      const beforeExpiry = sessionIncrementalStats();
      await cachedSessionSummary(runtime,path,parse);
      expect(sessionIncrementalStats().resumed-beforeExpiry.resumed).toBe(0);
      clearLedgerFactCache();
      const expired = await readTranscriptFacts(path,runtime);
      const expected = new TranscriptFacts(runtime,Date.now()-30*86400000);
      await parse(entry => expected.add(entry));
      expect(expired.facts).toEqual(expected.facts);
      expect(expired.facts.some(f => f.kind === 'user_message')).toBe(false);
    } finally { clock.mockRestore(); }
  });
  test(`${runtime}: restart resumes appended UTF-8/CRLF records and pairs earlier tools without rereading history`,async () => {
    const root = mkdtempSync(join(tmpdir(),'keepline-append-')); roots.push(root);const path = join(root,'session.jsonl');
    config.set('ledger',{ ...original,enabled: true,exclude: { projects: [],runtimes: [] } });
    const at = new Date().toISOString();
    const record = (kind: 'user' | 'call' | 'result' | 'reply',text = '做一个任务板') => runtime === 'codex'
      ? kind === 'user' ? { type: 'response_item',timestamp: at,payload: { type: 'message',role: 'user',content: [{ type: 'input_text',text }] } }
        : kind === 'call' ? { type: 'response_item',timestamp: at,payload: { type: 'function_call',call_id: 'c1',name: 'exec_command',arguments: '{"cmd":"bun test widget"}' } }
        : kind === 'result' ? { type: 'response_item',timestamp: at,payload: { type: 'function_call_output',call_id: 'c1',output: '{"exit_code":0,"output":"14 passed; 0 failed"}' } }
        : { type: 'response_item',timestamp: at,payload: { type: 'message',role: 'assistant',content: [{ type: 'output_text',text }] } }
      : { type: kind === 'reply' || kind === 'call' ? 'assistant' : 'user',sessionId: 'append-session',cwd: root,uuid: kind,timestamp: at,message: { role: kind === 'reply' || kind === 'call' ? 'assistant' : 'user',
          content: kind === 'call' ? [{ type: 'tool_use',id: 'c1',name: 'Bash',input: { command: 'bun test widget' } }]
            : kind === 'result' ? [{ type: 'tool_result',tool_use_id: 'c1',content: '14 passed; 0 failed',is_error: false }]
            : text } };
    const usage = (input: number) => runtime === 'codex'
      ? { type: 'event_msg',timestamp: at,payload: { type: 'token_count',model: 'gpt-5',usage: { input_tokens: input,output_tokens: 2 } } }
      : { type: 'assistant',sessionId: 'append-session',cwd: root,timestamp: at,message: { role: 'assistant',model: 'claude-sonnet-4-5',content: [],usage: { input_tokens: input,output_tokens: 2 } } };
    const header = runtime === 'codex'  ? [{ type: 'session_meta',timestamp: at,payload: { id: 'append-session',cwd: root } }] : [];
    // Large ignored records reproduce a live file whose historical outputs dwarf its new tail.
    const ignored = { type: runtime === 'codex' ? 'world_state' : 'file-history-snapshot',payload: 'x'.repeat(3*1024*1024) };
    writeFileSync(path,[...header,record('user'),ignored,usage(10),record('call')].map(r => JSON.stringify(r)).join('\r\n')+'\r\n');
    const parse = (onRecord?: (entry: unknown) => void,cursor?: JsonlCursorOptions) => runtime === 'codex'
      ? parseCodexSessionFile(path,{ includeToolCalls: false,onRecord,...cursor })
      : parseSessionFile(path,{ includeToolCalls: false,onRecord,...cursor });
    await cachedSessionSummary(runtime,path,parse);closeSessionSummaryCache();
    const before = sessionIncrementalStats();
    appendFileSync(path,JSON.stringify(record('result'))+'\r\n'+JSON.stringify(usage(3))+'\r\n'+JSON.stringify(record('reply','测试通过了 😀'))+'\r\n');
    const resumed = await cachedSessionSummary(runtime,path,parse);
    const after = sessionIncrementalStats();expect(after.resumed-before.resumed).toBe(1);expect(after.bytes-before.bytes).toBeLessThan(2000);
    expect(resumed).toEqual(await parse());
    expect(resumed?.usageStats?.totalInputTokens).toBe(13);expect(resumed?.usageStats?.apiCalls).toBe(2);
    const facts = await readTranscriptFacts(path,runtime);
    const call = facts.facts.find(f => f.kind === 'tool');expect(call?.kind).toBe('tool');
    if (call?.kind === 'tool') { expect(call.completed).toBe(true);expect(call.exitCode).toBe(0);expect(call.facts?.find(f => f.kind === 'test')?.exitCode).toBe(0); }
    // A valid non-newline tail is displayed, but its state must be replayed when the line completes.
    const tail = JSON.stringify(record('user','还要支持追加消息'));
    appendFileSync(path,tail);await cachedSessionSummary(runtime,path,parse);closeSessionSummaryCache();
    appendFileSync(path,'\r\n'+JSON.stringify(record('reply','追加完成'))+'\r\n');
    expect(await cachedSessionSummary(runtime,path,parse)).toEqual(await parse());
    clearLedgerFactCache();const secondFacts = await readTranscriptFacts(path,runtime);
    expect(secondFacts.facts.filter(f => f.kind === 'user_message' && f.text === '还要支持追加消息')).toHaveLength(1);
    const half = JSON.stringify(record('reply','半行 😀 后补齐'));
    const halfBytes = Buffer.from(half), split = halfBytes.indexOf(Buffer.from('😀'))+1;
    appendFileSync(path,halfBytes.subarray(0,split));await cachedSessionSummary(runtime,path,parse);closeSessionSummaryCache();
    appendFileSync(path,Buffer.concat([halfBytes.subarray(split),Buffer.from('\n')]));
    expect(await cachedSessionSummary(runtime,path,parse)).toEqual(await parse());
    // Truncation and inode replacement rebuild the summary rather than reuse stale counters.
    writeFileSync(path,[...header,record('user','替换了全部内容')].map(r => JSON.stringify(r)).join('\n')+'\n');
    expect(await cachedSessionSummary(runtime,path,parse)).toEqual(await parse());
    const replacement = join(root,'replacement');writeFileSync(replacement,[...header,record('user','另一份内容更长一些'),record('reply','替换后回复')].map(r => JSON.stringify(r)).join('\n')+'\n');renameSync(replacement,path);
    expect(await cachedSessionSummary(runtime,path,parse)).toEqual(await parse());
  });
}
