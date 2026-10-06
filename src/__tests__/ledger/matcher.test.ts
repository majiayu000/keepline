import { describe,test,expect } from 'bun:test';
import { DEFAULT_LEDGER_CONFIG,type TranscriptFact } from '../../domain/ledger/types.js';
import { constraintsFromText,decompose,extractAsks,matchLedger } from '../../domain/ledger/matcher.js';
import { sampleFacts } from './helpers.js';
describe('evidence matcher',() => {
  test('preserved failed tags flag removal but allow surrounding edits',() => {
    const facts = sampleFacts(); const items = decompose(extractAsks(facts,'s'),'s');
    items[0].source = 'user'; items[0].constraints = constraintsFromText('Keep old failed tags');
    const edit = { kind: 'tool' as const,callId: 'tags',name: 'Edit',input: { file_path: 'widget.ts',old_string: 'failed',new_string: 'passed' },mutating: true,at: new Date().toISOString() };
    facts.push(edit); expect(matchLedger(facts,items,[],[],DEFAULT_LEDGER_CONFIG).trail.at(-1)?.violations).toContain('Preserved text removed: failed');
    edit.input.new_string = 'failed (explained)'; expect(matchLedger(facts,items,[],[],DEFAULT_LEDGER_CONFIG).trail.at(-1)?.violations).toHaveLength(0);
  });
  test('successful anchored checks complete an item; failed checks do not',() => {
    const facts = sampleFacts(); const items = decompose(extractAsks(facts,'s'),'s',[{ id: 'check',text: 'Run `bun test src/widget.test.ts`' }]);
    expect(matchLedger(facts,items,[],[],DEFAULT_LEDGER_CONFIG).progress).toEqual({ done: 1,total: 1 });
    const tool = facts.find(f => f.kind === 'tool'); if (tool?.kind !== 'tool') throw new Error('No tool');
    tool.exitCode = 1; tool.facts!.forEach(f => { f.exitCode = 1 });
    const matched = matchLedger(facts,items,[],[],DEFAULT_LEDGER_CONFIG); expect(matched.progress.done).toBe(0); expect(matched.items[0].status).toBe('doing');
  });
  test('multi-part raw asks and keyword overlap never prove completion',() => {
    const facts = sampleFacts();
    const matched = matchLedger(facts,decompose(extractAsks(facts,'s'),'s'),[],[],DEFAULT_LEDGER_CONFIG);
    expect(matched.progress).toEqual({ done: 0,total: 0 }); expect(matched.items[0].status).toBe('todo');
    const items = decompose(extractAsks(facts,'s'),'s',[{ id: 'check',text: 'Fix widget and deployment' }]);
    expect(matchLedger(facts,items,[],[],DEFAULT_LEDGER_CONFIG).progress.done).toBe(0);
  });
  test('agent prose has no evidentiary value',() => {
    const facts = sampleFacts().filter(f => f.kind !== 'tool');
    const matched = matchLedger(facts,decompose(extractAsks(facts,'s'),'s'),[],[],DEFAULT_LEDGER_CONFIG); expect(matched.progress.done).toBe(0); expect(matched.evidence).toHaveLength(0);
  });
  test('deviation thresholds and correction suppression',() => {
    const facts: TranscriptFact[] = Array.from({ length: 8 },(_,i) => ({ kind: 'tool' as const,callId: `c${i}`,name: 'Write',input: { path: 'unrelated.ts' },mutating: true,at: new Date(i*100000).toISOString() }));
    const criteria = decompose([{ id: 'criterion',text: 'Work on widget',authoredText: 'Work on widget',at: new Date().toISOString(),kind: 'initial' }],'s').map(i => ({ ...i,source: 'user' as const }));
    expect(matchLedger(facts,[],[],[],DEFAULT_LEDGER_CONFIG).offPlan).toHaveLength(0);
    expect(matchLedger(facts,criteria,[],[],DEFAULT_LEDGER_CONFIG).offPlan).toHaveLength(1);
    expect(matchLedger(facts.slice(0,3),criteria,[],[],{ ...DEFAULT_LEDGER_CONFIG,deviation: 'sensitive' }).offPlan).toHaveLength(1);
    expect(matchLedger(facts,[],[],[],{ ...DEFAULT_LEDGER_CONFIG,deviation: 'off' }).offPlan).toHaveLength(0);
    expect(matchLedger(facts,[],facts.map(f => ({ callId: f.kind === 'tool' ? f.callId : '',acceptOffPlan: true })),[],DEFAULT_LEDGER_CONFIG).offPlan).toHaveLength(0);
  });
  test('constraints are checked independently and the toggle changes behavior',() => {
    const facts = sampleFacts(); const items = decompose(extractAsks(facts,'s'),'s'); items[0].source = 'user'; items[0].constraints = [{ kind: 'path_forbidden',value: 'src/**' }];
    facts.push({ kind: 'tool',callId: 'edit-1',name: 'Edit',input: { path: 'src/api.ts' },mutating: true,at: new Date().toISOString(),facts: [{ kind: 'file',value: 'src/api.ts' }] });
    expect(matchLedger(facts,items,[],[],DEFAULT_LEDGER_CONFIG).offPlan).toHaveLength(1);
    expect(matchLedger(facts,items,[],[],{ ...DEFAULT_LEDGER_CONFIG,constraints: false }).offPlan).toHaveLength(0);
  });
});
