import { describe,test,expect } from 'bun:test';
import { setupLedgerTest,seededLedger } from './helpers.js';
import { workItemRepository } from '../../infrastructure/database/repositories/work-item.repository.js';
import { acceptLedger,attributeLedger,getLedger } from '../../services/ledger/service.js';
import { goalRows,completeTodo } from '../../services/ledger/goals.js';
import { getDatabase } from '../../infrastructure/database/sqlite.js';
import { sessionRepository } from '../../infrastructure/database/repositories/session.repository.js';
describe('Keepline-native goals and todos',() => {
  setupLedgerTest();
  test('create, attribute, verify, accept and roll up without external upserts',async () => {
    const goal = workItemRepository.create({ title: 'Ship widget',level: 'goal',outcome: 'Widget works' });
    const todo = workItemRepository.create({ title: 'Widget tests',parentId: goal.id,acceptance: [{ id: 'test-check',text: 'Run bun test src/widget.test.ts',completed: false }] });
    const detail = await seededLedger(); await attributeLedger(detail.sessionId,todo.id);
    const linked = (await getLedger(detail.sessionId))!; expect(linked.items[0].source).toBe('work_item');
    expect(goalRows()[0].todos[0].checklist[0]).toMatchObject({ evidenced: true,satisfied: false });
    expect(goalRows()[0].todos[0].readyToComplete).toBe(false);
    await acceptLedger(detail.sessionId,{ decision: 'accepted',droppedItemIds: [] });
    expect(goalRows()[0].todos[0].readyToComplete).toBe(true); expect(goalRows()[0].progress.done).toBe(0);
    completeTodo(todo.id); expect(goalRows()[0].progress).toMatchObject({ done: 1,total: 1 });
    expect(goalRows(undefined,new Date(),goal.id)[0].recent.some(e => e.kind === 'completed' && e.todoId === todo.id)).toBe(true);
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
  test('project maps use confirmed links and real evidence, and expose hook waiting reasons',async () => {
    const goal = workItemRepository.create({ title: 'Ship',level: 'goal' });
    const todo = workItemRepository.create({ title: 'Widget',parentId: goal.id,acceptance: [{ id: 'check',text: 'Run bun test src/widget.test.ts',completed: false }] });
    const detail = await seededLedger(); await attributeLedger(detail.sessionId,todo.id);
    await getLedger(detail.sessionId);
    sessionRepository.upsert({ sessionId: detail.sessionId,status: 'needs_input',statusSource: 'hook',statusReason: '等待批准测试命令' });
    expect(goalRows()[0].recent).toEqual([]);
    const map = goalRows(undefined,new Date(),goal.id)[0];
    expect(map.todos[0].sessions[0]).toMatchObject({ needsInput: true,statusReason: '等待批准测试命令' });
    expect(map.progress.active).toBe(1);
    expect(map.recent.some(e => e.kind === 'test' && e.sessionId === detail.sessionId)).toBe(true);
    expect(map.recent.some(e => e.title.includes('Completed:'))).toBe(false);
    getDatabase().query("UPDATE work_item_session_links SET acceptance_status='pending'").run();
    const unlinked = goalRows(undefined,new Date(),goal.id)[0];
    expect(unlinked.todos[0].sessions).toEqual([]);
    expect(unlinked.todos[0].checklist[0].evidenced).toBe(false);
    expect(unlinked.recent).toEqual([]);
  });
  test('missing evidence and scan-inferred waiting do not become verified work or approval requests',async () => {
    const goal = workItemRepository.create({ title: 'Ship',level: 'goal' });
    const todo = workItemRepository.create({ title: 'Widget',parentId: goal.id,acceptance: [{ id: 'check',text: 'Run bun test src/widget.test.ts',completed: false }] });
    const detail = await seededLedger(); await attributeLedger(detail.sessionId,todo.id); await getLedger(detail.sessionId);
    sessionRepository.upsert({ sessionId: detail.sessionId,status: 'waiting',statusSource: 'scan' });
    getDatabase().query('DELETE FROM progress_evidence').run();
    const map = goalRows(undefined,new Date(),goal.id)[0];
    expect(map.todos[0].sessions[0].needsInput).toBe(false);
    expect(map.todos[0].checklist[0]).toMatchObject({ evidenced: false,satisfied: false });
    expect(map.todos[0].readyToComplete).toBe(false);
    expect(map.recent).toEqual([]);
  });
  test('recent progress is bounded to real events in seven days, not work-item edits or failed checks',async () => {
    const goal = workItemRepository.create({ title: 'Ship',level: 'goal' });
    const todo = workItemRepository.create({ title: 'Widget',parentId: goal.id });
    const detail = await seededLedger(); await attributeLedger(detail.sessionId,todo.id);
    const db = getDatabase();
    db.query("UPDATE progress_evidence SET outcome='failed'").run();
    expect(goalRows(undefined,new Date(),goal.id)[0].recent).toEqual([]);
    db.query("UPDATE progress_evidence SET outcome='completed',occurred_at='2000-01-01T00:00:00.000Z'").run();
    workItemRepository.update(todo.id,{ title: 'Edited today' });
    expect(goalRows(undefined,new Date(),goal.id)[0].recent).toEqual([]);
  });
});
