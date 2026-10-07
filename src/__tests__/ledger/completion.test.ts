import { describe, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
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
  test.each(['Run bun test.', 'Run bun test, then report.'])('plain command prose preserves the executed criterion: %s', (text) => {
    const items = decompose([], 'plain-command', [{ id: 'tests', text }]);
    expect(items[0].anchors.commands).toEqual(['bun test']);
    expect(matchLedger(recordedCalls([{ cmd: 'bun test', code: 0 }]), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(1);
  });

  test('explicit command delimiters preserve punctuation and environment assignments', () => {
    const items = decompose([], 'quoted-command', [{ id: 'tests', text: 'Run `CI=1 bun test src/widget.test.ts`.' }]);
    expect(items[0].anchors.commands).toEqual(['CI=1 bun test src/widget.test.ts']);
    expect(matchLedger(recordedCalls([{ cmd: 'bun test src/widget.test.ts', code: 0 }]), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(0);
  });

  test.each([
    "sed -i 's/a/b/' src/private/a.ts src/public/b.ts",
    "sed -i.bak -e 's/a/b/' src/private/a.ts src/public/b.ts",
    "sed -n -i '' 's/a/b/p' src/private/a.ts src/public/b.ts",
    "sed --in-place=.bak --expression='s/a/b/' -- src/private/a.ts src/public/b.ts",
    "sed -ni.bak -f edits.sed 'src/private/a.ts' 'src/public/b.ts'",
  ])('all in-place sed file operands participate in forbidden-path checks: %s', (cmd) => {
    const items = decompose([], 'sed-files', [{ id: 'edit', text: 'Do not edit src/private/**' }]);
    const result = matchLedger(recordedCalls([{ cmd, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG);
    expect(result.evidence.filter(e => e.kind === 'file').map(e => e.value)).toEqual(['src/private/a.ts', 'src/public/b.ts']);
    expect(result.trail[0].violations).toEqual(['Forbidden path: src/private/**']);
  });

  test.each(['write_stdin', 'functions.write_stdin'])('nonempty %s input invalidates checks from before the write', (name) => {
    const start = { cmd: 'cat', result: { session_id: 91, output: 'Process running with session ID 91' } };
    const input = { name, input: { session_id: 91, chars: 'edit files\n' }, result: { session_id: 91, output: '' } };
    const stale = match([start, { code: 0 }, input]);
    expect(stale.progress.done).toBe(0);
    expect(stale.items[0].status).toBe('unverified');
    expect(stale.items[0].evidenceIds).toEqual([]);
    expect(match([start, { code: 0 }, input, { code: 0 }]).progress.done).toBe(1);
    expect(match([start, { code: 0 }, { ...input, input: { session_id: 91, chars: '' } }]).progress.done).toBe(1);
  });

  test('a real successful multi-file sed edit exposes every changed file', () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-multiple-edits-'));
    const cmd = "sed -i.bak 's/a/b/' 'private file.ts' public.ts";
    try {
      for (const path of ['private file.ts', 'public.ts']) writeFileSync(join(root, path), 'a\n');
      const result = spawnSync('sh', ['-c', cmd], { cwd: root, encoding: 'utf8' });
      if (result.error) throw result.error;
      expect(result.status).toBe(0);
      for (const path of ['private file.ts', 'public.ts']) expect(readFileSync(join(root, path), 'utf8')).toBe('b\n');
      const facts = recordedCalls([{ cmd, result: { exit_code: result.status, output: result.stdout + result.stderr } }]);
      const items = decompose([], 'real-sed', [{ id: 'edit', text: 'Edit public.ts' }]);
      items[0].constraints = [{ kind: 'path_forbidden', value: 'private file.ts' }];
      const matched = matchLedger(facts, items, [], [], DEFAULT_LEDGER_CONFIG);
      expect(matched.evidence.filter(e => e.kind === 'file').map(e => e.value)).toEqual(['private file.ts', 'public.ts']);
      expect(matched.trail[0].violations).toEqual(['Forbidden path: private file.ts']);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('environment assignments remain part of the exact command criterion', () => {
    expect(match([{ cmd: 'CI=1 bun test', code: 0 }], ['CI=1 bun test']).progress.done).toBe(1);
    expect(match([{ cmd: 'bun test', code: 0 }], ['CI=1 bun test']).progress.done).toBe(0);
  });

  test('direct shell mutation invalidates earlier checks and a rerun restores them', () => {
    const mutation = { cmd: 'git checkout -- src/widget.ts', code: 0, output: '' };
    const stale = match([{ code: 0 }, mutation]);
    expect(stale.items[0].status).toBe('unverified');
    expect(stale.items[0].evidenceIds).toEqual([]);
    expect(match([{ code: 0 }, mutation, { code: 0 }]).progress.done).toBe(1);
  });

  test('independent current command criteria retain their own receipts', () => {
    expect(match([{ code: 0 }, { cmd: 'bun run typecheck', code: 0 }], [command, 'bun run typecheck']).progress.done).toBe(1);
  });

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

  test.each([
    'sed -i s/a/b/ src/widget.ts',
    'sed -i.bak s/a/b/ src/widget.ts',
    'sed --in-place s/a/b/ src/widget.ts',
    'sed --in-place=.bak s/a/b/ src/widget.ts',
    'sed -ni s/a/b/p src/widget.ts',
    'sed -n -i s/a/b/p src/widget.ts',
    `sed "-""i".bak 's/a/b/' src/widget.ts`,
    `sed -e 's/a/b/' -i.bak src/widget.ts`,
  ])('in-place edit attempts require fresh checks regardless of exit status: %s', (cmd) => {
    const edit = { cmd, output: '' };
    expect(match([{ code: 0 }, { ...edit, code: 0 }]).progress.done).toBe(0);
    expect(match([{ code: 0 }, { ...edit, code: 1 }]).progress.done).toBe(0);
    expect(match([{ code: 0 }, edit]).progress.done).toBe(0);
    expect(match([{ code: 0 }, { ...edit, code: 0 }, { code: 0 }]).progress.done).toBe(1);
  });

  test('a read-only sed failure preserves earlier successful checks', () => {
    expect(match([{ code: 0 }, { cmd: 'sed -n 1,3p missing.ts', code: 1, output: 'not found' }])
      .progress.done).toBe(1);
  });

  test.each([
    `sed -n 's/x/ -i/p' widget.ts`,
    `sed -n 's/x/ -i /p' widget.ts`,
    `sed -n "s/x/ -i/p" widget.ts`,
    `sed -n -e '-i' widget.ts`,
    `sed -n -f '-i' widget.ts`,
    `sed -n --expression '-i' widget.ts`,
    `sed -n --file '-i' widget.ts`,
    `sed -n --expression=-i widget.ts`,
    `sed -n 's/x/y/p' widget.ts # -i`,
    `sed -- -i widget.ts`,
  ])('option-like text is never in-place edit evidence: %s', (cmd) => {
    for (const code of [undefined, 0, 1]) {
      const result = match([{ code: 0 }, { cmd, code, output: '' }]);
      expect(result.progress.done).toBe(1);
      expect(result.evidence.filter(e => e.callId === 'completion-1' && e.kind === 'file')).toEqual([]);
    }
  });

  test.each([
    'sed -i s/a/b/ "$TARGET"',
    `sed -n -i 's/a/b/p' "$TARGET"`,
    `sed -ni 's/a/b/p' "$TARGET"`,
    'sed -i s/a/b/ src/widget.ts || true',
  ])('uncertain sed execution invalidates checks without proving a file write: %s', (cmd) => {
    const result = match([{ code: 0 }, { cmd, code: 0, output: '' }]);
    expect(result.progress.done).toBe(0);
    expect(result.items[0].status).toBe('unverified');
    expect(result.evidence.filter(e => e.callId === 'completion-1' && e.kind === 'file')).toEqual([]);
  });

  test.each([
    `sed -i.bak 's/a/b /' widget.ts missing.ts`,
    `sed -n -i.bak 's/a/b /p' "$TARGET" missing.ts`,
    `sed -ni.bak 's/a/b /p' "$TARGET" missing.ts`,
    `cat widget.ts\nsed -i.bak 's/a/b /' widget.ts missing.ts`,
  ])('a failed multi-file in-place edit invalidates a real earlier check: %s', (editCommand) => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-partial-edit-'));
    const execute = (cmd: string, args: string[]) => {
      const result = spawnSync(cmd, args, { cwd: root, encoding: 'utf8', env: { ...process.env, TARGET: 'widget.ts' } });
      if (result.error) throw result.error;
      if (result.status === null) throw new Error(`Command stopped before an exit receipt: ${cmd}`);
      return { exit_code: result.status, output: result.stdout + result.stderr };
    };
    try {
      expect(execute('git', ['init', '--quiet']).exit_code).toBe(0);
      writeFileSync(join(root, 'widget.ts'), 'a\n');
      expect(execute('git', ['add', 'widget.ts']).exit_code).toBe(0);
      const before = execute('git', ['diff', '--check']);
      expect(before.exit_code).toBe(0);
      // A backup suffix works with both GNU and BSD sed. The existing first
      // file is changed before the missing second file makes the command fail.
      const edit = execute('sh', ['-c', editCommand]);
      expect(edit.exit_code).not.toBe(0);
      expect(readFileSync(join(root, 'widget.ts'), 'utf8')).toBe('b \n');
      expect(execute('git', ['diff', '--check']).exit_code).not.toBe(0);
      const calls = [
        { cmd: 'git diff --check', result: before },
        { cmd: editCommand, result: edit },
      ];
      // Only the original success and failed edit enter the transcript. The
      // independently failed recheck above must not be needed to invalidate it.
      const stale = match(calls, ['git diff --check']);
      expect(stale.progress).toEqual({ done: 0, total: 1 });
      expect(stale.items[0].status).toBe('unverified');
      expect(stale.items[0].evidenceIds).toEqual([]);
      expect(stale.evidence.filter(e => e.callId === 'completion-1' && e.kind === 'file')).toEqual([]);
      expect(stale.evidence.filter(e => e.callId === 'completion-1').every(e => e.exitCode !== 0)).toBe(true);

      writeFileSync(join(root, 'widget.ts'), 'b\n');
      const current = execute('git', ['diff', '--check']);
      expect(current.exit_code).toBe(0);
      const verified = match([...calls, { cmd: 'git diff --check', result: current }], ['git diff --check']);
      expect(verified.progress).toEqual({ done: 1, total: 1 });
      expect(verified.evidence.filter(e => verified.items[0].evidenceIds.includes(e.id))
        .every(e => e.callId === 'completion-2')).toBe(true);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
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

  test.each(['', '2 pass'])('a later chunk %j cannot erase an observed test failure', (output) => {
    const start = { result: { session_id: 91, output: '1 pass\n1 fail' } };
    const chunk = { name: 'write_stdin', input: { session_id: 91, chars: '' } };
    const calls = [start, { ...chunk, result: { session_id: 91, output: 'still running' } }, { ...chunk, code: 0, output }];
    expect(match(calls).progress.done).toBe(0);
    // A separate execution can still establish a new successful check.
    expect(match([...calls, { code: 0 }]).progress.done).toBe(1);
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
