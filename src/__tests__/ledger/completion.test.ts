import { describe, expect, test } from 'bun:test';
import { TranscriptFacts } from '../../adapters/transcript-facts.js';
import { decompose, matchLedger } from '../../domain/ledger/matcher.js';
import { DEFAULT_LEDGER_CONFIG } from '../../domain/ledger/types.js';

const command = 'bun test src/widget.test.ts';
type Call = { cmd?: string; name?: string; input?: unknown; code?: number; output?: string; result?: unknown };

function recordedCalls(calls: Call[]) {
  const parser = new TranscriptFacts('codex');
  calls.forEach((call, index) => {
    const timestamp = new Date(Date.UTC(2026, 9, 6, 16, 0, index)).toISOString();
    const call_id = `completion-${index}`;
    parser.add({ type: 'response_item', timestamp, payload: {
      type: 'function_call', call_id, name: call.name ?? 'exec_command',
      arguments: JSON.stringify(call.input ?? { cmd: call.cmd ?? command }),
    } });
    if (call.code !== undefined || call.result !== undefined) parser.add({ type: 'response_item', timestamp, payload: {
      type: 'function_call_output', call_id,
      output: JSON.stringify(call.result ?? { exit_code: call.code, output: call.output ?? (call.code === 0 ? '1 pass' : '1 fail') }),
    } });
  });
  return parser.facts;
}

function match(calls: Call[], commands = [command]) {
  const items = decompose([], 'completion-session', [{
    id: 'tests', text: commands.map(cmd => `Ensure \`${cmd}\` passes`).join(' and '),
  }]);
  return matchLedger(recordedCalls(calls), items, [], [], DEFAULT_LEDGER_CONFIG, 'completion-session');
}

describe('completion requires current execution evidence', () => {
  test('a directly recorded successful command proves its literal criterion', () => {
    expect(match([{ code: 0 }]).progress).toEqual({ done: 1, total: 1 });
  });

  test.each([
    [`echo "${command}"`, command],
    [`printf '%s\\n' '${command}'`, command],
    [`${command} || true`, '1 fail'],
    ['bun test src/widgetXtestXts', '1 pass'],
  ])('text overlap in %s is not command execution proof', (cmd, output) => {
    expect(match([{ cmd, code: 0, output }]).progress.done).toBe(0);
  });

  test('an unexecuted command inside a code wrapper cannot borrow its exit code', () => {
    const result = match([{
      name: 'functions.exec', input: { input: `if (false) await tools.exec_command({cmd: "${command}"});` },
      code: 0, output: '1 pass',
    }]);
    expect(result.progress.done).toBe(0);
  });

  test('a failing test summary cannot borrow a zero process exit code', () => {
    expect(match([{ code: 0, output: '1 pass\n1 fail' }]).progress.done).toBe(0);
  });

  test('a later failure replaces the earlier success for the same command', () => {
    const result = match([{ code: 0 }, { code: 1 }]);
    expect(result.progress.done).toBe(0);
    expect(result.items[0].evidenceIds).toEqual([]);
  });

  test('a later pending attempt also requires a new successful result', () => {
    expect(match([{ code: 0 }, {}]).progress.done).toBe(0);
  });

  test.each([
    { cmd: `${command} || true`, code: 0, output: '1 fail' },
    { cmd: `${command} || true` },
    { name: 'functions.exec', input: { input: `await tools.exec_command({cmd: "${command}"});` }, code: 1 },
    { name: 'functions.exec', input: { input: `await tools.exec_command({cmd: "${command}"});` } },
    { name: 'functions.exec', input: { input: `if (false) await tools.exec_command({cmd: "${command}"});` }, code: 0 },
  ] satisfies Call[])('an uncertain later execution invalidates old successful checks: %j', (call) => {
    const stale = match([{ code: 0 }, call]);
    expect(stale.progress.done).toBe(0);
    expect(stale.items[0].status).toBe('unverified');
    expect(stale.items[0].evidenceIds).toEqual([]);
    expect(match([{ code: 0 }, call, { code: 0 }]).progress.done).toBe(1);
  });

  test('a later success restores completion after a failed attempt', () => {
    const result = match([{ code: 1 }, { code: 0 }]);
    expect(result.progress.done).toBe(1);
    const completion = result.evidence.filter(e => result.items[0].evidenceIds.includes(e.id));
    expect(completion.every(e => e.callId === 'completion-1')).toBe(true);
  });

  test('an observed edit invalidates prior checks until they run again', () => {
    const edit = { name: 'apply_patch', input: { input: '*** Begin Patch\n*** Update File: src/widget.ts\n@@\n-old\n+new\n*** End Patch' }, code: 0, output: 'Success. Updated src/widget.ts' };
    const stale = match([{ code: 0 }, edit]);
    expect(stale.progress.done).toBe(0);
    expect(stale.items[0].status).toBe('unverified');
    expect(stale.items[0].evidenceIds).toEqual([]);
    expect(match([{ code: 0 }, edit, { code: 0 }]).progress.done).toBe(1);
  });

  test('read-only shell checks retain command receipts without becoming steps', () => {
    const result = match([{ cmd: 'git diff --check', code: 0, output: '' }], ['git diff --check']);
    expect(result.progress.done).toBe(1);
    expect(result.readOnlyCount).toBe(1);
    expect(result.trail).toHaveLength(0);
    expect(match([{ cmd: 'git diff --check', code: 1, output: '' }], ['git diff --check']).progress.done).toBe(0);
  });

  test('an explicitly failed edit without file evidence preserves successful checks', () => {
    expect(match([{ code: 0 }, { name: 'Edit', input: { file_path: 'src/widget.ts' },
      result: { exit_code: 1, is_error: true, output: 'old_string not found' } }]).progress.done).toBe(1);
    expect(match([{ code: 0 }, { name: 'Edit', input: { file_path: 'src/widget.ts' },
      result: { is_error: true, output: 'old_string not found' } }]).progress.done).toBe(1);
    expect(match([{ code: 0 }, { name: 'Edit', input: { file_path: 'src/widget.ts' } }]).progress.done).toBe(0);
  });

  test('successful shell in-place edits invalidate checks, failed edits do not', () => {
    const edit = { cmd: 'sed -i s/a/b/ src/widget.ts', output: '' };
    expect(match([{ code: 0 }, { ...edit, code: 0 }]).progress.done).toBe(0);
    expect(match([{ code: 0 }, { ...edit, code: 1 }]).progress.done).toBe(1);
    expect(match([{ code: 0 }, { ...edit, code: 0 }, { code: 0 }]).progress.done).toBe(1);
  });

  test('async terminal completion belongs to the original command and session', () => {
    const start = { result: { session_id: 91, output: 'Process running with session ID 91' } };
    const poll = { name: 'write_stdin', input: { session_id: 91, chars: '' } };
    expect(match([start, { ...poll, code: 0 }]).progress.done).toBe(1);
    expect(match([start, { ...poll, code: 1 }]).progress.done).toBe(0);
    expect(match([start, { ...poll, result: { session_id: 91, output: 'still running' } }]).progress.done).toBe(0);
    expect(match([start, { ...poll, input: { session_id: 92, chars: '' }, code: 0 }]).progress.done).toBe(0);
    expect(match([start, { ...poll, code: 0 }, { ...poll, code: 1 }]).progress.done).toBe(0);
  });

  test('every command in a criterion needs current successful evidence', () => {
    const second = 'bun test src/integration.test.ts';
    expect(match([{ code: 0 }], [command, second]).progress.done).toBe(0);
    expect(match([{ code: 0 }, { cmd: second, code: 0 }], [command, second]).progress.done).toBe(1);
    expect(match([{ code: 0 }, { cmd: second, code: 0 }, { code: 1 }], [command, second]).progress.done).toBe(0);
  });

  test('reassignment cannot hide the latest failed check from its criterion', () => {
    const facts = recordedCalls([{ code: 0 }, { code: 1 }]);
    const items = decompose([], 's', [{ id: 'check', text: `Run \`${command}\`` }, { id: 'other', text: 'Review documentation' }]);
    const result = matchLedger(facts, items, [{ callId: 'completion-1', itemId: items[1].id }], [], DEFAULT_LEDGER_CONFIG, 's');
    expect(result.items[0].status).not.toBe('done');
    expect(result.progress.done).toBe(0);
  });
});
