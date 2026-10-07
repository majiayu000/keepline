import { describe, test, expect } from 'bun:test';
import { parseCodexSessionFile } from '../../adapters/codex/parser.js';
import { parseSessionFile } from '../../adapters/claude/parser/jsonl.js';
import { isMutatingTool, TranscriptFacts } from '../../adapters/transcript-facts.js';
import { cachedSessionSummary } from '../../infrastructure/session-summary-cache.js';
import { readTranscriptFacts,clearLedgerFactCache,ledgerFactCacheStats } from '../../services/ledger/facts.js';
import { mkdtempSync,writeFileSync,utimesSync,rmSync,copyFileSync,appendFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { matchLedger, decompose, extractAsks } from '../../domain/ledger/matcher.js';
import { DEFAULT_LEDGER_CONFIG } from '../../domain/ledger/types.js';
const fixtures = `${import.meta.dir}/fixtures`;
describe('normalized transcript facts',() => {
  test('reading a saved test log is not a successful test execution',() => {
    const parser = new TranscriptFacts('claude'),timestamp = new Date().toISOString();
    parser.add({ type: 'assistant',timestamp,message: { content: [{ type: 'tool_use',id: 'read',name: 'Read',input: { file_path: 'test.log' } }] } });
    parser.add({ type: 'user',timestamp,message: { content: [{ type: 'tool_result',tool_use_id: 'read',content: 'Exit code: 0\n3 pass',is_error: false }] } });
    expect(parser.facts[0]).toMatchObject({ kind: 'tool',exitCode: undefined,facts: [] });
  });
  test('system notifications do not start a user turn',() => {
    const parser = new TranscriptFacts('claude'),timestamp = new Date().toISOString();
    parser.add({ type: 'user',uuid: 'notice',timestamp,message: { content: '<task-notification>Agent completed</task-notification>' } });
    expect(parser.facts).toHaveLength(0);
  });
  test('shell reads of saved test output cannot prove execution',() => {
    for (const runtime of ['claude','codex'] as const) {
      const parser = new TranscriptFacts(runtime),timestamp = new Date().toISOString();
      if (runtime === 'claude') {
        parser.add({ type: 'assistant',timestamp,message: { content: [{ type: 'tool_use',id: 'read',name: 'Bash',input: { command: 'cat test.log' } }] } });
        parser.add({ type: 'user',timestamp,message: { content: [{ type: 'tool_result',tool_use_id: 'read',content: 'Exit code: 0\n3 passed; 0 failed',is_error: false }] } });
      } else {
        parser.add({ type: 'response_item',timestamp,payload: { type: 'function_call',call_id: 'read',name: 'exec_command',arguments: '{"cmd":"cat test.log"}' } });
        parser.add({ type: 'response_item',timestamp,payload: { type: 'function_call_output',call_id: 'read',output: 'Exit code: 0\n3 passed; 0 failed' } });
      }
      expect(parser.facts[0]).toMatchObject({ kind: 'tool',mutating: false,facts: [{ kind: 'command',value: 'cat test.log',exitCode: 0 }] });
    }
  });
  test('expired files are not opened; large fact sets survive cache eviction through disk',async () => {
    const root = mkdtempSync(join(tmpdir(),'keepline-large-facts-')),path = join(root,'large.jsonl');
    try {
      clearLedgerFactCache();
      writeFileSync(path,'not JSON'); const old = new Date(Date.now()-60*86400000); utimesSync(path,old,old);
      const before = ledgerFactCacheStats().reads;
      expect((await readTranscriptFacts(path,'codex')).facts).toHaveLength(0);
      expect(ledgerFactCacheStats().reads).toBe(before);
      writeFileSync(path,JSON.stringify({ type: 'response_item',timestamp: new Date().toISOString(),payload: { type: 'function_call',call_id: 'large',name: 'exec_command',arguments: JSON.stringify({ cmd: 'echo '+ 'x'.repeat(17*1024*1024) }) } })+'\n');
      expect((await readTranscriptFacts(path,'codex')).facts).toHaveLength(1);
      expect(ledgerFactCacheStats().bytes).toBe(0);
      const cached = ledgerFactCacheStats(); clearLedgerFactCache();
      expect((await readTranscriptFacts(path,'codex')).facts).toHaveLength(1);
      expect(ledgerFactCacheStats().reads).toBe(cached.reads);
      expect(ledgerFactCacheStats().diskHits).toBe(cached.diskHits+1);
    } finally { clearLedgerFactCache(); rmSync(root,{ recursive: true }); }
  });
  test('changed summary scans stream facts once and discard their arrays before returning',async () => {
    const root = mkdtempSync(join(tmpdir(),'ledger-one-pass-'));
    try {
      for (const runtime of ['codex','claude'] as const) {
        const path = join(root,`${runtime}.jsonl`); copyFileSync(`${fixtures}/${runtime}-cli.jsonl`,path);
        const parse = (onRecord?: (entry: unknown) => void) => runtime === 'codex' ? parseCodexSessionFile(path,{ includeToolCalls: false,onRecord }) : parseSessionFile(path,{ includeToolCalls: false,onRecord });
        const summary = await cachedSessionSummary(runtime,path,parse);
        expect(summary?.transcriptFacts).toBeUndefined();
        const before = ledgerFactCacheStats(); clearLedgerFactCache();
        expect((await readTranscriptFacts(path,runtime)).facts.length).toBeGreaterThan(0);
        expect(ledgerFactCacheStats().reads).toBe(before.reads);
        appendFileSync(path,JSON.stringify({ type: runtime === 'codex' ? 'event_msg' : 'user',timestamp: new Date().toISOString(),payload: { type: 'user_message',message: 'Also fix next.ts' },message: { content: 'Also fix next.ts' },cwd: '/tmp/project' })+'\n');
        await cachedSessionSummary(runtime,path,parse); clearLedgerFactCache();
        expect((await readTranscriptFacts(path,runtime)).facts.some(f => f.kind === 'user_message' && f.text === 'Also fix next.ts')).toBe(true);
        expect(ledgerFactCacheStats().reads).toBe(before.reads);
      }
    } finally { clearLedgerFactCache(); rmSync(root,{ recursive: true }); }
  });
  test('summary scans never collect transcript facts, even on a cold cache',async () => {
    for (const parsed of [await parseCodexSessionFile(`${fixtures}/codex-cli.jsonl`,{ includeToolCalls: false }),await parseSessionFile(`${fixtures}/claude-cli.jsonl`,{ includeToolCalls: false })]) expect(parsed?.transcriptFacts).toBeUndefined();
  });
  test('Claude success without an exit-code string produces evidenced completion; errors and background tasks do not',() => {
    for (const [output,is_error,background,code] of [['3 pass',false,false,0],['1 fail',true,false,1],['Bash running in background',false,true,undefined]] as const) {
      const parser = new TranscriptFacts('claude'); const timestamp = new Date().toISOString();
      parser.add({ type: 'user',timestamp,uuid: 'turn',message: { content: 'Run `bun test widget`' } });
      parser.add({ type: 'assistant',timestamp,message: { content: [{ type: 'tool_use',id: 'bash',name: 'Bash',input: { command: 'bun test widget',run_in_background: background } }] } });
      parser.add({ type: 'user',timestamp,message: { content: [{ type: 'tool_result',tool_use_id: 'bash',content: output,is_error }] } });
      expect(parser.facts.find(f => f.kind === 'tool')).toMatchObject({ exitCode: code });
      const matched = matchLedger(parser.facts,decompose(extractAsks(parser.facts,'claude'),'claude',[{ id: 'check',text: 'Run `bun test widget`' }]),[],[],DEFAULT_LEDGER_CONFIG);
      expect(matched.progress.done).toBe(code === 0 ? 1 : 0);
    }
  });
  test('test counts distinguish zero failures and prevent partial success from proving a check',() => {
    for (const [output,done] of [['14 passed; 0 failed',1],['3 passed; 1 failed',0]] as const) {
      const parser = new TranscriptFacts('codex'),timestamp = new Date().toISOString();
      parser.add({ type: 'response_item',timestamp,payload: { type: 'function_call',name: 'exec_command',call_id: 'tests',arguments: '{"cmd":"bun test widget"}' } });
      parser.add({ type: 'item_completed',timestamp,item: { type: 'CommandExecution',id: 'tests',exit_code: 0,aggregated_output: output } });
      const matched = matchLedger(parser.facts,decompose([],'test',[{ id: 'check',text: 'Run `bun test widget`' }]),[],[],DEFAULT_LEDGER_CONFIG);
      expect(matched.progress.done).toBe(done);
      expect(matched.evidence.filter(e => e.kind === 'test').every(e => e.exitCode === (done ? 0 : 1))).toBe(true);
    }
  });
  test('structured CommandExecution is authoritative; mixed wrapper exits cannot prove success',() => {
    const parser = new TranscriptFacts('codex'); const timestamp = new Date().toISOString();
    const add = (payload: unknown,type = 'response_item') => parser.add({ type,timestamp,payload });
    add({ type: 'function_call',call_id: 'x',name: 'exec_command',arguments: '{"cmd":"bun test && cargo build"}' });
    add({ type: 'function_call_output',call_id: 'x',output: 'Exit code: 0\n3 pass\nExit code: 101\nbuild failed' });
    expect(parser.facts[0]).toMatchObject({ exitCode: 101 });
    add({ item: { type: 'CommandExecution',id: 'x',exit_code: 101,aggregated_output: 'Exit code: 0\n3 pass\nbuild failed' } },'item_completed');
    expect(parser.facts[0]).toMatchObject({ exitCode: 101 });
    const tool = parser.facts[0]; if (tool.kind === 'tool') expect(tool.facts?.every(e => e.exitCode === 101)).toBe(true);
    add({ item: { type: 'CommandExecution',id: 'standalone',command: 'cargo test',exit_code: 0,aggregated_output: 'test result: ok' } },'item_completed');
    expect(parser.facts[1]).toMatchObject({ exitCode: 0 });
  });
  test('retention filters individual records and known Codex metadata stays available',() => {
    const parser = new TranscriptFacts('codex',Date.now()-86400000);
    parser.add({ type: 'response_item',timestamp: new Date(Date.now()-2*86400000).toISOString(),payload: { type: 'message',role: 'user',content: 'Old ask' } });
    for (const type of ['token_usage_record','world_state','inter_agent_communication_metadata','compacted']) parser.add({ type,timestamp: new Date().toISOString() });
    expect(parser.facts).toHaveLength(0); expect(parser.unknownRecords).toBe(0);
  });
  for (const runtime of ['codex-cli','codex-desktop','claude-cli']) test(`${runtime}: calls pair with results and turns`,async () => {
    const parsed = runtime.startsWith('codex') ? await parseCodexSessionFile(`${fixtures}/${runtime}.jsonl`) : await parseSessionFile(`${fixtures}/${runtime}.jsonl`);
    const facts = parsed!.transcriptFacts!;
    const tool = facts.find(f => f.kind === 'tool' && f.callId === 'c1');
    expect(tool).toMatchObject({ exitCode: 1,mutating: true });
    expect(facts.filter(f => f.kind === 'user_message')).toHaveLength(1);
    expect(facts.some(f => f.kind === 'turn' && f.phase === 'completed')).toBe(true);
    if (tool?.kind === 'tool') expect(tool.facts).toContainEqual({ kind: 'test',value: '1 fail',exitCode: 1 });
  });
  test('commit and PR facts come from tool output',async () => {
    const parsed = await parseCodexSessionFile(`${fixtures}/codex-cli.jsonl`);
    const tool = parsed!.transcriptFacts!.find(f => f.kind === 'tool' && f.callId === 'c2');
    if (tool?.kind !== 'tool') throw new Error('Missing tool');
    expect(tool.facts?.find(e => e.kind === 'commit')?.value).toContain('abc1234');
    expect(tool.facts?.find(e => e.kind === 'pr')?.value).toBe('https://github.com/example/project/pull/42');
  });
  test('read-only commands collapse, combined writes remain steps',() => {
    for (const cmd of ['cat a.ts','git status','git diff','git -C repo status --short','git --no-pager log -1','git --no-pager -C "repo path" diff --check','git -Crepo status --short','rg widget src','sed -n 1,3p a.ts']) expect(isMutatingTool('Bash',{ command: cmd })).toBe(false);
    for (const cmd of ['cat a.ts > b.ts','git status && git commit -m fix','git -C repo status && git -C repo checkout .','git --no-pager checkout .','git -C repo commit -m fix','git --unknown status','git -C','bun test','sed -i s/a/b/ a.ts']) expect(isMutatingTool('Bash',{ command: cmd })).toBe(true);
    expect(isMutatingTool('functions.find',{ command: 'find . -delete' })).toBe(true);
    expect(isMutatingTool('functions.wait',{})).toBe(false);
    expect(isMutatingTool('Bash',{ command: 'sleep 60' })).toBe(false);
    expect(isMutatingTool('TodoWrite',{ todos: [] })).toBe(false);
    for (const command of ['find . -delete','find . -exec rm {} +','find . -fprint result.txt']) expect(isMutatingTool('Bash',{ command })).toBe(true);
    expect(isMutatingTool('functions.exec',{ input: 'text(await tools.exec_command({cmd: "rg widget src"}));' })).toBe(false);
    expect(isMutatingTool('functions.exec',{ input: 'text(await tools.exec_command({cmd: "bun test"}));' })).toBe(true);
    expect(isMutatingTool('functions.exec',{ input: 'text(await tools.write_stdin({session_id: 1,chars: ""}));' })).toBe(false);
  });
  test('prefixed Git reads use direct executable normalization without guessing wrappers',() => {
    for (const cmd of ['CI=1 /usr/bin/git status --short','env -- CI=1 git -C repo diff --check','/usr/bin/env CI=1 /usr/bin/git --no-pager -Crepo log -1']) expect(isMutatingTool('exec_command',{ cmd })).toBe(false);
    for (const cmd of ['env --unknown git status','sh -c "git status"','CI=1 git --unknown status','CI=1 git -C']) expect(isMutatingTool('exec_command',{ cmd })).toBe(true);
  });
  test.each(['read_text_file','list_directory','mcp__filesystem__read_text_file','mcp__filesystem__list_directory'])(
    '%s is read-only and cannot turn copied test output into evidence',name => {
      expect(isMutatingTool(name,{ path: 'src/widget.ts' })).toBe(false);
      const parser = new TranscriptFacts('codex'),timestamp = new Date().toISOString();
      parser.add({ type: 'response_item',timestamp,payload: { type: 'function_call',call_id: 'read',name,arguments: '{"path":"src/widget.ts"}' } });
      parser.add({ type: 'response_item',timestamp,payload: { type: 'function_call_output',call_id: 'read',output: 'Exit code: 0\n3 pass' } });
      expect(parser.facts[0]).toMatchObject({ mutating: false,facts: [] });
    });
  test.each(['mcp__filesystem__read_and_write_file','read_text_file_extra','list_directory_extra'])(
    '%s is not classified as a read',name => {
      expect(isMutatingTool(name,{ path: 'src/widget.ts' })).toBe(true);
    });
  test('MCP exec terminal completion moves a pending mutation after a mid-flight check',() => {
    const parser = new TranscriptFacts('codex');
    const add = (payload: unknown,second: number) => parser.add({ type: 'response_item',timestamp: new Date(Date.UTC(2026,9,8,1,0,second)).toISOString(),payload });
    add({ type: 'function_call',call_id: 'write',name: 'mcp__shell__exec_command',arguments: '{"cmd":"node write.js"}' },0);
    add({ type: 'function_call_output',call_id: 'write',output: '{"session_id":91,"output":"running"}' },1);
    add({ type: 'function_call',call_id: 'check',name: 'exec_command',arguments: '{"cmd":"bun test"}' },2);
    add({ type: 'function_call_output',call_id: 'check',output: '{"exit_code":0,"output":"1 pass"}' },3);
    add({ type: 'function_call',call_id: 'poll',name: 'write_stdin',arguments: '{"session_id":91,"chars":""}' },4);
    add({ type: 'function_call_output',call_id: 'poll',output: '{"exit_code":0,"output":""}' },5);
    expect(parser.facts.at(-1)).toMatchObject({ callId: 'write',name: 'mcp__shell__exec_command',exitCode: 0,at: '2026-10-08T01:00:05.000Z' });
    const items = decompose([],'mcp-async',[{ id: 'check',text: 'Run `bun test`' }]);
    expect(matchLedger(parser.facts,items,[],[],DEFAULT_LEDGER_CONFIG).items[0].status).toBe('unverified');
  });
  test('unknown and malformed records degrade individually',() => {
    const parser = new TranscriptFacts('codex'); parser.add({ type: 'future',timestamp: new Date().toISOString() }); parser.add(null);
    expect(parser.unknownRecords).toBe(1);
  });
  test('disk summary cache excludes raw facts, memory fact cache restores them',async () => {
    const path = `${fixtures}/codex-cli.jsonl`;
    await cachedSessionSummary('codex',path,() => parseCodexSessionFile(path));
    const cached = await cachedSessionSummary('codex',path,() => { throw new Error('Unexpected parse'); });
    expect(cached?.transcriptFacts).toBeUndefined();
    expect(cached?.sourcePath).toBe(path);
    expect((await readTranscriptFacts(path,'codex')).facts.length).toBeGreaterThan(0);
  });
  test('Codex custom tool calls and structured item results pair',() => {
    const parser = new TranscriptFacts('codex'); const at = new Date().toISOString();
    parser.add({ type: 'response_item',timestamp: at,payload: { type: 'custom_tool_call',call_id: 'x',name: 'apply_patch',input: '*** Update File: src/widget.ts\n+export const x = 1' } });
    parser.add({ type: 'response_item',timestamp: at,payload: { type: 'custom_tool_call_output',call_id: 'x',output: 'Success. Updated files:\nM src/widget.ts' } });
    expect(parser.facts[0]).toMatchObject({ callId: 'x',facts: [{ kind: 'file',value: 'src/widget.ts' }] });
  });
});
