import { describe,test,expect } from 'bun:test';
import { decompose,extractAsks,followUpSuggestions,constraintsFromText } from '../../domain/ledger/matcher.js';
import { sampleFacts } from './helpers.js';
describe('asks and decomposition',() => {
  test('raw asks yield constraints without a model and apply to a linked checklist',() => {
    const text = 'Do not change public API.\nDo not edit src/private/**\nKeep old failed tags';
    expect(constraintsFromText(text)).toEqual([{ kind: 'no_public_api_change' },{ kind: 'path_forbidden',value: 'src/private/**' },{ kind: 'preserve_text',value: 'failed' }]);
    const items = decompose([{ id: 'ask',text,authoredText: text,kind: 'initial',at: new Date().toISOString() }],'s',[{ id: 'check',text: 'Run tests' }]);
    expect(items[0].constraints).toHaveLength(3);
  });
  test('only final proposals become follow-up suggestions',() => {
    const at = new Date().toISOString();
    expect(followUpSuggestions([{ kind: 'agent_message',at,final: true,text: 'Tests passed\n**Next steps:**\n- Add smoke tests\n- Update docs\n\nEvidence:\n- bun test\nNext: Add monitoring' },{ kind: 'agent_message',at,final: false,text: 'Next: Delete everything' }])).toEqual(['Add smoke tests','Update docs','Add monitoring']);
  });
  test('verbatim asks distinguish additions from approvals',() => {
    const facts = sampleFacts(); const at = new Date().toISOString();
    facts.push({ kind: 'user_message',text: '  Also keep the public API.\nPlease.  ',at },{ kind: 'user_message',text: 'okay',at });
    const asks = extractAsks(facts,'a'); expect(asks).toHaveLength(2); expect(asks[1].text).toBe('  Also keep the public API.\nPlease.  '); expect(asks[1].kind).toBe('addition');
    expect(decompose(asks,'a')).toHaveLength(2);
  });
  test('unrestricted user intent is kept; questions stay verbatim without adding progress items',() => {
    const at = new Date().toISOString();
    const texts = ['我想做一个 agent 任务板','我要求功能和性能都比他们好','体验再顺手一点','这个库还有什么没做的吗','为什么没有修复?','Run tests?'];
    const asks = extractAsks(texts.map(text => ({ kind: 'user_message',text,at })),'s');
    expect(asks.map(a => a.text)).toEqual(texts);
    expect(asks.slice(3).every(a => a.kind === 'question')).toBe(true);
    expect(decompose(asks,'s').map(i => i.title)).toEqual(texts.slice(0,3));
    const mixed = extractAsks([{ kind: 'user_message',text: '我想做一个 agent 任务板\n有什么建议？',at },{ kind: 'user_message',text: 'import the previous requirements',at }],'s');
    expect(mixed.every(ask => ask.kind !== 'question')).toBe(true); expect(decompose(mixed,'s')).toHaveLength(2);
    const inline = extractAsks([{ kind: 'user_message',text: '请补充 README.md 的安装说明。顺便解释一下为什么需要这个步骤？',at }],'s');
    expect(inline[0].kind).toBe('initial');
    expect(decompose(inline,'s')).toHaveLength(1);
  });
  test('only explicit non-requests are excluded, with authored text outside pasted blocks preserved',() => {
    const at = new Date().toISOString();
    const messages = ['<codex_internal_context source="goal">continue goal</codex_internal_context>','<send_user_message_question_reply>[{"answer":"yes"}]</send_user_message_question_reply>','<task-notification>agent completed</task-notification>','现在呢','How is it going?','okay','好','继续','bun test','git status\nbun run typecheck','```ts\nconst fixes = ["修复", "实现"];\n```','<pasted_content>已完成修复和实现</pasted_content>',JSON.stringify({ command: '实现' })];
    const facts = messages.map(text => ({ kind: 'user_message' as const,text,at }));
    facts.push({ kind: 'user_message',text: '请修复 scanner.ts，并保留公共接口',at });
    expect(extractAsks(facts,'s').map(a => a.text)).toEqual(['请修复 scanner.ts，并保留公共接口']);
    expect(extractAsks([{ kind: 'user_message',text: '<task-notification>done</task-notification>\n修复 widget.ts',at }],'s')[0].text).toBe('修复 widget.ts');
    const mixed = '照这个修改\n<pasted_content>修复、实现、删除</pasted_content>\n性能再好一点';
    const ask = extractAsks([{ kind: 'user_message',text: mixed,at }],'s')[0];
    expect(ask.text).toBe(mixed);
    expect(ask.authoredText).toBe('照这个修改\n\n性能再好一点');
    expect(decompose([ask],'s')[0].anchors.keywords).not.toContain('删除');
  });
  test('a copied completion report does not become requirements, but its surrounding instruction does',() => {
    const at = new Date().toISOString();
    const report = `## 结论\n\n已修复、实现了下面这些功能。${'细节说明。'.repeat(30)}\n\n## 完成的检查\n\n| 检查 | 结果 |\n| --- | --- |\n| bun test | 593 个测试全部通过 |\n\n未提交或推送。`;
    expect(extractAsks([{ kind: 'user_message',text: report,at }],'s')).toEqual([]);
    const asks = extractAsks([{ kind: 'user_message',text: `按这个验收\n${report}`,at }],'s');
    expect(asks).toHaveLength(1); expect(asks[0].authoredText).toBe('按这个验收');
  });
  test('todo checklist takes precedence without a model',() => {
    const items = decompose(extractAsks(sampleFacts(),'a'),'a',[{ id: 'criterion-1',text: 'Run `bun test`' }]);
    expect(items).toHaveLength(1); expect(items[0]).toMatchObject({ source: 'work_item',checklistId: 'criterion-1' });
  });
  test('questions ending in 呢 remain visible without creating requirements',() => {
    const at = new Date().toISOString();
    const texts = ['是在网页还是 app 呢','你看看这是什么呢','你觉得应该怎么做呢？'];
    const asks = extractAsks(texts.map(text => ({ kind: 'user_message',text,at })),'s');
    expect(asks.map(a => a.text)).toEqual(texts);
    expect(asks.every(a => a.kind === 'question')).toBe(true);
    expect(decompose(asks,'s')).toEqual([]);
    expect(extractAsks([{ kind: 'user_message',text: '保留轨迹\n你觉得应该怎么做呢',at }],'s')[0].kind).toBe('initial');
  });
});
