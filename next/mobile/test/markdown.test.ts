import { expect, test } from 'bun:test';
import { blocks } from '../src/ui/blocks';

test('an Agent reply splits into the blocks a bubble draws', () => {
  const md = [
    '## 结论',
    '还差两件事：',
    '1. 手机端轮询',
    '   - 每 2 秒',
    '2. 断线补齐',
    '',
    '```rust',
    'fn main() {}',
    '',
    '```',
    '> 引用一行',
    '> 第二行',
    '---',
    '收尾 `code` 和 **粗体**',
  ].join('\n');
  expect(blocks(md)).toEqual([
    { kind: 'heading', level: 2, text: '结论' },
    { kind: 'para', text: '还差两件事：' },
    { kind: 'item', mark: '1.', depth: 0, text: '手机端轮询' },
    { kind: 'item', mark: '•', depth: 1, text: '每 2 秒' },
    { kind: 'item', mark: '2.', depth: 0, text: '断线补齐' },
    { kind: 'code', text: 'fn main() {}\n' },
    { kind: 'quote', text: '引用一行\n第二行' },
    { kind: 'rule' },
    { kind: 'para', text: '收尾 `code` 和 **粗体**' },
  ]);
});

test('an unclosed fence keeps the rest as code', () => {
  expect(blocks('看这个\n```\na\nb')).toEqual([
    { kind: 'para', text: '看这个' },
    { kind: 'code', text: 'a\nb' },
  ]);
});
