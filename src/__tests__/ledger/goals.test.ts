import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger } from './helpers.js';
import { workItemRepository } from '../../infrastructure/database/repositories/work-item.repository.js';
import { acceptLedger,attributeLedger,getLedger } from '../../services/ledger/service.js';
import { goalRows,completeTodo } from '../../services/ledger/goals.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
describe('Keepline-native goals and todos',() => {
  setupLedgerTest();
  test('create, attribute, verify, accept and roll up without external upserts',async () => {
    const goal = workItemRepository.create({ title: 'Ship widget',level: 'goal',outcome: 'Widget works' });
    const todo = workItemRepository.create({ title: 'Widget tests',parentId: goal.id,acceptance: [{ id: 'test-check',text: 'Run bun test src/widget.test.ts',completed: false }] });
    const detail = await seededLedger(); await attributeLedger(detail.sessionId,todo.id);
    const linked = (await getLedger(detail.sessionId))!; expect(linked.items[0].source).toBe('work_item');
    expect(goalRows()[0].todos[0].readyToComplete).toBe(false);
    await acceptLedger(detail.sessionId,{ decision: 'accepted',droppedItemIds: [] });
    expect(goalRows()[0].todos[0].readyToComplete).toBe(true); expect(goalRows()[0].progress.done).toBe(0);
    completeTodo(todo.id); expect(goalRows()[0].progress).toMatchObject({ done: 1,total: 1 });
  });
  test('hierarchy rejects invalid parents, duplicate checklist ids and deleting a goal with todos',() => {
    const goal = workItemRepository.create({ title: 'Goal',level: 'goal' });
    const todo = workItemRepository.create({ title: 'Todo',parentId: goal.id });
    expect(() => workItemRepository.create({ title: 'Bad',level: 'goal',parentId: goal.id })).toThrow('cannot have a parent');
    expect(() => workItemRepository.create({ title: 'Bad',parentId: todo.id })).toThrow('parent must be a goal');
    expect(() => workItemRepository.delete(goal.id)).toThrow('child todos');
    expect(() => workItemRepository.update(todo.id,{ acceptance: [{ id: 'x',text: '',completed: false }] })).toThrow('Checklist');
  });
  test('stale goals reflect edits and activity, not only todo completion',() => {
    workItemRepository.create({ title: 'Old goal',level: 'goal' });
    getDatabase().query("UPDATE work_items SET updated_at='2000-01-01T00:00:00.000Z'").run();
    expect(goalRows()[0].stale).toBe(true);
  });
});
