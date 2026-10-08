import { describe, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { TranscriptFacts } from '../../adapters/transcript-facts.js';
import { decompose, matchLedger } from '../../domain/ledger/matcher.js';
import { DEFAULT_LEDGER_CONFIG } from '../../domain/ledger/types.js';

const command = 'bun test src/widget.test.ts';
// GNU information options are absent from BSD sed; hook receipts below are portable.
const sedVersion = spawnSync('sed', ['--version'], { encoding: 'utf8' });
const hasGnuSed = sedVersion.status === 0 && /GNU sed/.test(sedVersion.stdout);
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

  for (const subcommand of ['diff', 'log', 'show']) for (const separated of [false, true]) {
    test(`real git ${subcommand} output-file ${separated ? 'operand' : 'equals'} invalidates prior checks`, () => {
      const root = mkdtempSync(join(tmpdir(), 'keepline-git-output-'));
      const git = (...args: string[]) => {
        const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
        if (result.error) throw result.error;
        return result;
      };
      try {
        expect(git('init', '--quiet').status).toBe(0);
        writeFileSync(join(root, 'widget.ts'), 'before\n');
        expect(git('add', 'widget.ts').status).toBe(0);
        expect(git('-c', 'user.name=Ledger regression', '-c', 'user.email=ledger@example.invalid', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', 'commit', '--quiet', '-m', 'fixture').status).toBe(0);
        writeFileSync(join(root, 'widget.ts'), 'after\n');
        const args = [subcommand, ...(subcommand === 'diff' ? ['--exit-code'] : []), ...(separated ? ['--output', 'generated.ts'] : ['--output=generated.ts'])];
        const receipt = git(...args);
        expect(receipt.status).toBe(subcommand === 'diff' ? 1 : 0);
        expect(readFileSync(join(root, 'generated.ts'), 'utf8').length).toBeGreaterThan(0);
        const cmd = `git ${args.join(' ')}`;
        expect(match([{ code: 0 }, { cmd, result: { exit_code: receipt.status, output: receipt.stdout + receipt.stderr } }]).items[0].status).toBe('unverified');
        expect(match([{ code: 0 }, { cmd, code: 0 }, { code: 0 }]).progress.done).toBe(1);
      } finally { rmSync(root, { recursive: true, force: true }); }
    });
  }

  test.each(['git diff -- --output=generated.ts', 'git log -- --output generated.ts', 'git show -- --output=generated.ts'])(
    'Git output-looking paths after -- remain reads: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).progress.done).toBe(1);
    });

  test.each(['git branch --show-current', 'CI=1 git --no-pager -C repo branch --show-current'])(
    'exact current-branch inspection preserves prior checks: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).progress.done).toBe(1);
    });
  test.each(['git branch -D old', 'git branch new', 'git branch --show-current -D old'])(
    'other branch forms remain mutations: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).items[0].status).toBe('unverified');
    });

  test.each(['-u', '--update-snapshots'])('real Bun snapshot mode %s invalidates peers but proves its own criterion', option => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-snapshot-update-'));
    try {
      const path = join(root, 'widget.test.ts');
      const writeTest = (value: string) => writeFileSync(path, `import { test, expect } from 'bun:test';\ntest('value', () => expect('${value}').toMatchSnapshot());\n`);
      writeTest('before');
      expect(spawnSync('bun', ['test', 'widget.test.ts'], { cwd: root, encoding: 'utf8' }).status).toBe(0);
      const snapshot = join(root, '__snapshots__', 'widget.test.ts.snap');
      expect(readFileSync(snapshot, 'utf8')).toContain('before');
      writeTest('after');
      const receipt = spawnSync('bun', ['test', option, 'widget.test.ts'], { cwd: root, encoding: 'utf8' });
      if (receipt.error) throw receipt.error;
      expect(receipt.status).toBe(0);
      expect(readFileSync(snapshot, 'utf8')).toContain('after');
      const cmd = `bun test ${option} widget.test.ts`;
      const calls = [{ code: 0 }, { cmd, result: { exit_code: receipt.status, output: receipt.stdout + receipt.stderr } }];
      const items = decompose([], 'snapshot-mode', [{ id: 'peer', text: `Run \`${command}\`` }, { id: 'snapshot', text: `Run \`${cmd}\`` }]);
      expect(matchLedger(recordedCalls(calls), items, [], [], DEFAULT_LEDGER_CONFIG).items.map(item => item.status)).toEqual(['unverified', 'done']);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  test('real pnpm run options locate the script after an option operand', () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-pnpm-run-'));
    try {
      writeFileSync(join(root, 'package.json'), JSON.stringify({ scripts: { test: `node -e "console.log('1 pass')"` } }));
      const args = ['run', '--if-present', '--dir', root, 'test'];
      const receipt = spawnSync('pnpm', args, { cwd: root, encoding: 'utf8' });
      if (receipt.error) throw receipt.error;
      expect(receipt.status).toBe(0);
      expect(receipt.stdout).toContain('1 pass');
      const cmd = `pnpm run --if-present --dir ${root} test`;
      expect(match([{ code: 0 }, { cmd, result: { exit_code: receipt.status, output: receipt.stdout + receipt.stderr } }], [command, cmd]).progress.done).toBe(1);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  test.each(['pnpm run --if-present test', 'pnpm run --if-present --dir "fixture path" test', 'pnpm run --dir="fixture path" --if-present test', 'pnpm run --no-bail test'])(
    'recognized package run options preserve peer checks: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0 }], [command, cmd]).progress.done).toBe(1);
    });
  test('npm if-present runs the actual script and preserves peer checks', () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-npm-if-present-'));
    try {
      writeFileSync(join(root, 'package.json'), JSON.stringify({ scripts: { test: `node -e "console.log('1 pass')"` } }));
      const receipt = spawnSync('npm', ['run', '--if-present', 'test'], { cwd: root, encoding: 'utf8' });
      if (receipt.error) throw receipt.error;
      expect(receipt.status).toBe(0);
      expect(receipt.stdout).toContain('1 pass');
      const cmd = 'npm run --if-present test';
      expect(match([{ code: 0 }, { cmd, result: { exit_code: receipt.status, output: receipt.stdout + receipt.stderr } }], [command, cmd]).progress.done).toBe(1);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });
  test.each([
    'bun test --reporter=junit --reporter-outfile=src/widget.ts',
    'npm test -- --reporter=junit --reporter-outfile=src/widget.ts',
    'bun test --reporter=junit --reporter-outfile src/widget.ts',
    'bun test --coverage --coverage-reporter=lcov',
    'bun test --coverage --coverage-reporter lcov --coverage-dir src',
  ])('test report output invalidates earlier evidence: %s', cmd => {
    expect(match([{ code: 0 }, { cmd, code: 0 }]).items[0].status).toBe('unverified');
    expect(match([{ cmd, code: 0 }], [cmd]).progress.done).toBe(1);
  });
  test('Bun actually writes the explicit reporter destination', () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-bun-reporter-'));
    try {
      writeFileSync(join(root, 'pass.test.ts'), "import {test,expect} from 'bun:test'; test('pass',()=>expect(1).toBe(1));\n");
      const dest = join(root, 'report.xml');
      writeFileSync(dest, 'old source bytes');
      const receipt = spawnSync(process.execPath, ['test', 'pass.test.ts', '--reporter=junit', '--reporter-outfile', dest], { cwd: root, encoding: 'utf8' });
      if (receipt.error) throw receipt.error;
      expect(receipt.status).toBe(0);
      expect(readFileSync(dest, 'utf8')).toContain('<testsuites');
      const cmd = `bun test pass.test.ts --reporter=junit --reporter-outfile=${dest}`;
      expect(match([{ code: 0 }, { cmd, result: { exit_code: receipt.status, output: receipt.stdout + receipt.stderr } }]).items[0].status).toBe('unverified');
    } finally { rmSync(root, { recursive: true, force: true }); }
  });
  test('report flag-looking filename after separator preserves peer evidence', () => {
    expect(match([{ code: 0 }, { cmd: 'bun test -- --reporter-outfile=src/widget.ts', code: 0 }]).progress.done).toBe(1);
  });

  test.each(['pnpm run --unknown test', 'pnpm run --dir', 'pnpm run --report-summary test'])(
    'unknown, incomplete or writing run modes remain conservative: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0 }]).items[0].status).toBe('unverified');
    });

  test.each([command, 'git diff --check', 'bun test -u widget.test.ts'])(
    'a check started before a later edit stays stale after terminal success: %s', cmd => {
      const start = { cmd, result: { session_id: 91, output: 'running' } };
      const edit = { name: 'Edit', input: { file_path: 'src/widget.ts', old_string: 'old', new_string: 'new' }, code: 0, output: '' };
      const poll = { name: 'write_stdin', input: { session_id: 91, chars: '' }, code: 0, output: '' };
      expect(match([start, poll], [cmd]).progress.done).toBe(1);
      expect(match([start, edit, poll], [cmd]).items[0].status).toBe('unverified');
      expect(match([start, edit, poll, { cmd, code: 0 }], [cmd]).progress.done).toBe(1);
      expect(match([edit, start, poll], [cmd]).progress.done).toBe(1);
    });
  test.each([
    'git diff --output=src/widget.ts',
    'git diff --output src/widget.ts',
    'CI=1 /usr/bin/git --no-pager -C . log --output=src/widget.ts',
    'env -- CI=1 git show --output=src/widget.ts',
  ])('review boundary: Git output modes invalidate previous checks: %s', cmd => {
    const calls: Call[] = [{ code: 0 }, { cmd, code: 0, output: '' }];
    const stale = match(calls, [command, cmd]);
    expect(stale.items[0].status).toBe('unverified');
    expect(stale.progress.done).toBe(0);
    expect(match([...calls, { code: 0 }], [command, cmd]).progress.done).toBe(1);
  });

  test.each(['git diff --output-indicator-new=+', 'git diff -- --output=src/widget.ts', 'git log --format=--output=src/widget.ts'])(
    'review boundary: Git output-like data remains read-only: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).progress.done).toBe(1);
    });

  test.each(['git branch --show-current', 'CI=1 /usr/bin/git --no-pager -C . branch --show-current'])(
    'review boundary: explicit branch inspection preserves checks: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: 'main' }], [command, cmd]).progress.done).toBe(1);
    });
  test.each(['git branch new-branch', 'git branch -d old-branch', 'git branch --show-current -m new-branch'])(
    'review boundary: branch changes stay mutating: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).items[0].status).toBe('unverified');
    });

  test.each(['bun test --update-snapshots', 'bun test -u', 'bun test snapshot.test.ts --update-snapshots', 'npm test -- -u', 'pnpm run test --update-snapshots'])(
    'review boundary: snapshot-updating criteria invalidate previous checks: %s', cmd => {
      const calls: Call[] = [{ code: 0 }, { cmd, code: 0 }];
      expect(match(calls, [command, cmd]).items[0].status).toBe('unverified');
      expect(match([...calls, { code: 0 }], [command, cmd]).progress.done).toBe(1);
    });

  test.each(['pnpm run --if-present test', 'pnpm --silent run test', 'pnpm --filter widget run --if-present test', 'pnpm run --resume-from widget check'])(
    'review boundary: supported package-manager options preserve peer checks: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0 }], [command, cmd]).progress.done).toBe(1);
    });
  test.each(['pnpm run --unknown test', 'pnpm run --resume-from test write'])(
    'review boundary: unknown options and option operands are not verification scripts: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0 }]).items[0].status).toBe('unverified');
    });

  test('review boundary: a real Git output file makes the prior check stale', () => {
    const root = mkdtempSync(join(tmpdir(), 'ledger-git-output-'));
    const git = (...args: string[]) => spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    const check = () => spawnSync(process.execPath, ['test', 'check.test.ts'], { cwd: root, encoding: 'utf8' });
    try {
      expect(git('init', '-b', 'main').status).toBe(0);
      writeFileSync(join(root, 'widget.txt'), 'before\n');
      expect(git('add', 'widget.txt').status).toBe(0);
      expect(git('-c', 'user.name=Ledger Test', '-c', 'user.email=ledger@example.invalid', 'commit', '-m', 'fixture').status).toBe(0);
      writeFileSync(join(root, 'widget.txt'), 'after\n');
      writeFileSync(join(root, 'check.test.ts'), "import {test,expect} from 'bun:test'; import {readFileSync} from 'node:fs'; test('current file',()=>expect(readFileSync('widget.txt','utf8')).toBe('after\\n'));\n");
      const before = check(); expect(before.status).toBe(0);
      const changed = git('diff', '--output=widget.txt'); expect(changed.status).toBe(0);
      expect(readFileSync(join(root, 'widget.txt'), 'utf8')).toContain('diff --git');
      expect(check().status).toBe(1);
      const cmd = 'bun test check.test.ts';
      const stale = match([{ cmd, code: 0, output: before.stdout + before.stderr }, { cmd: 'git diff --output=widget.txt', code: 0, output: changed.stdout }], [cmd]);
      expect(stale.items[0].status).toBe('unverified');
      expect(stale.items[0].evidenceIds).toEqual([]);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  test('review boundary: a real snapshot update invalidates an earlier file check', () => {
    const root = mkdtempSync(join(tmpdir(), 'ledger-snapshot-update-'));
    const run = (args: string[], value = 'before') => spawnSync(process.execPath, args, { cwd: root, encoding: 'utf8', env: { ...process.env, SNAPSHOT_VALUE: value } });
    try {
      writeFileSync(join(root, 'snapshot.test.ts'), "import {test,expect} from 'bun:test'; test('value',()=>expect(process.env.SNAPSHOT_VALUE).toMatchSnapshot());\n");
      expect(run(['test', '--update-snapshots', 'snapshot.test.ts']).status).toBe(0);
      writeFileSync(join(root, 'check.test.ts'), "import {test,expect} from 'bun:test'; import {readFileSync} from 'node:fs'; test('original snapshot',()=>expect(readFileSync('__snapshots__/snapshot.test.ts.snap','utf8')).toContain('before'));\n");
      const before = run(['test', 'check.test.ts']); expect(before.status).toBe(0);
      const updated = run(['test', '-u', 'snapshot.test.ts'], 'after'); expect(updated.status).toBe(0);
      expect(readFileSync(join(root, '__snapshots__/snapshot.test.ts.snap'), 'utf8')).toContain('after');
      expect(run(['test', 'check.test.ts']).status).toBe(1);
      const cmd = 'bun test check.test.ts';
      const stale = match([{ cmd, code: 0, output: before.stdout + before.stderr }, { cmd: 'SNAPSHOT_VALUE=after bun test -u snapshot.test.ts', code: 0, output: updated.stdout + updated.stderr }], [cmd]);
      expect(stale.items[0].status).toBe('unverified');
      expect(stale.items[0].evidenceIds).toEqual([]);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  test.each(['exec_command', 'mcp__shell__exec_command'])(
    'review boundary: a check started before an edit cannot prove the edited state: %s', name => {
      const calls: Call[] = [
        { name, result: { session_id: 91, output: 'running' } },
        { name: 'apply_patch', input: { input: '*** Update File: src/widget.ts\n-old\n+new' }, code: 0, output: 'Success' },
        { name: 'write_stdin', input: { session_id: 91, chars: '' }, code: 0 },
      ];
      const stale = match(calls);
      expect(stale.items[0].status).toBe('unverified');
      expect(stale.progress.done).toBe(0);
      expect(stale.items[0].evidenceIds).toEqual([]);
      expect(match([...calls, { code: 0 }, calls[2]]).progress.done).toBe(1);
    });

  test('review boundary: equal timestamps preserve start/edit/end ordering', () => {
    const parser = new TranscriptFacts('codex'), timestamp = '2026-10-08T00:00:00.000Z';
    const add = (payload: unknown) => parser.add({ type: 'response_item', timestamp, payload });
    add({ type: 'function_call', call_id: 'check', name: 'exec_command', arguments: JSON.stringify({ cmd: command }) });
    add({ type: 'function_call', call_id: 'edit', name: 'apply_patch', arguments: '{"input":"*** Update File: src/widget.ts"}' });
    add({ type: 'function_call_output', call_id: 'edit', output: '{"exit_code":0,"output":"Success"}' });
    add({ type: 'function_call_output', call_id: 'check', output: '{"exit_code":0,"output":"1 pass"}' });
    const items = decompose([], 'overlap', [{ id: 'check', text: `Run \`${command}\`` }]);
    expect(matchLedger(parser.facts, items, [], [], DEFAULT_LEDGER_CONFIG).items[0].status).toBe('unverified');
    add({ type: 'function_call', call_id: 'fresh', name: 'exec_command', arguments: JSON.stringify({ cmd: command }) });
    add({ type: 'function_call_output', call_id: 'fresh', output: '{"exit_code":0,"output":"1 pass"}' });
    expect(matchLedger(parser.facts, items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(1);
  });

  test.each([0, 1])('review boundary: a late older result cannot replace a newer retry: %s', code => {
    const calls: Call[] = [
      { result: { session_id: 91, output: 'running' } },
      { name: 'apply_patch', input: { input: '*** Update File: src/widget.ts' }, code: 0, output: 'Success' },
      { code },
      { name: 'write_stdin', input: { session_id: 91, chars: '' }, code: 0 },
    ];
    expect(match(calls).progress.done).toBe(code === 0 ? 1 : 0);
  });

  test.each([
    'LC_ALL=C rg foo src',
    '/usr/bin/rg foo src',
    '/usr/bin/env -- LC_ALL=C /usr/bin/rg foo src',
  ])('normalized read commands preserve completed checks: %s', cmd => {
    const result = match([{ code: 0 }, { cmd, code: 0, output: 'src/widget.ts' }]);
    expect(result.items[0].status).toBe('done');
    expect(result.readOnlyCount).toBe(1);
  });

  test.each([
    'LC_ALL=C find src -delete',
    '/usr/bin/find src -exec rm {} +',
    '/usr/bin/env -- LC_ALL=C /usr/bin/find src -fprint listing.txt',
  ])('normalized mutating find still invalidates checks: %s', cmd => {
    expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).items[0].status).toBe('unverified');
  });

  test.each([
    { code: 1 },
    { result: { session_id: 91, output: 'running' } },
  ])('attempted readonly criterion is doing until success: %j', result => {
    const cmd = 'git diff --check';
    const observed = match([{ cmd, ...result }], [cmd]);
    expect(observed.items[0].status).toBe('doing');
    expect(observed.progress.done).toBe(0);
    expect(observed.trail).toHaveLength(0);
    expect(match([{ cmd, ...result }, { cmd, code: 0, output: '' }], [cmd]).items[0].status).toBe('done');
  });

  test('partial readonly criteria are doing without inventing complete success', () => {
    const first = 'git status --short', second = 'git diff --check';
    const result = match([{ cmd: first, code: 0, output: '' }], [first, second]);
    expect(result.items[0].status).toBe('doing');
    expect(result.items[0].evidenceIds.length).toBeGreaterThan(0);
    expect(result.progress.done).toBe(0);
    expect(match([{ cmd: first, code: 0, output: '' }, { cmd: second, code: 0, output: '' }], [first, second]).items[0].status).toBe('done');
  });

  test.each(['src/file?', 'src/file[ab]?', 'src/file?.'])(
    'plain command criteria retain glob operands: %s', path => {
      const cmd = `git ls-files ${path}`;
      const items = decompose([], 'glob-criteria', [{ id: 'glob', text: `Run ${cmd}` }]);
      expect(items[0].anchors.commands).toEqual([cmd]);
      expect(matchLedger(recordedCalls([{ cmd, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(1);
    });

  test.each([
    "LC_ALL=C sed -i.bak 's/a/b/' src/private/a.ts",
    "LC_ALL='C' LANG=C /usr/bin/sed -i.bak 's/a/b/' src/private/a.ts",
    "/usr/bin/sed -i.bak 's/a/b/' src/private/a.ts",
    "env LC_ALL=C sed -i.bak 's/a/b/' src/private/a.ts",
    "/usr/bin/env -- LC_ALL=C /usr/bin/sed -i.bak 's/a/b/' src/private/a.ts",
  ])('prefixed sed exposes reliable operands to forbidden paths: %s', cmd => {
    const items = decompose([], 'prefixed-sed', [{ id: 'private', text: 'Do not edit src/private/**' }]);
    const result = matchLedger(recordedCalls([{ cmd, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG);
    expect(result.evidence.filter(e => e.kind === 'file').map(e => e.value)).toEqual(['src/private/a.ts']);
    expect(result.trail.flatMap(t => t.violations)).toEqual(['Forbidden path: src/private/**']);
    expect(matchLedger(recordedCalls([{ cmd, code: 1, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG).evidence.filter(e => e.kind === 'file')).toEqual([]);
  });

  test('prefixed ambiguous BSD/GNU sed keeps mutation without guessed file facts', () => {
    const cmd = "LC_ALL=C /usr/bin/sed -i .bak 's@/src/private/a.ts@new@' public.ts";
    const result = match([{ code: 0 }, { cmd, code: 0, output: '' }]);
    expect(result.items[0].status).toBe('unverified');
    expect(result.evidence.filter(e => e.kind === 'file')).toEqual([]);
  });

  test.each([0, 1])('a mutation completion invalidates a check run while pending: %s', code => {
    const start = { cmd: 'node write.js', result: { session_id: 91, output: 'running' } };
    const pending = { name: 'write_stdin', input: { session_id: 91, chars: '' }, result: { session_id: 91, output: 'still running' } };
    const completed = { name: 'write_stdin', input: { session_id: 91, chars: '' }, code, output: '' };
    const calls: Call[] = [start, { code: 0 }, pending];
    expect(match(calls).progress.done).toBe(1);
    const stale = match([...calls, completed]);
    expect(stale.items[0].status).toBe('unverified');
    const facts = recordedCalls([...calls, completed]);
    expect(facts.filter(f => f.kind === 'tool')).toHaveLength(4);
    expect(facts.at(-1)).toMatchObject({ kind: 'tool', callId: 'completion-0', name: 'exec_command', at: '2026-10-06T16:00:03.000Z' });
    expect(facts.find(f => f.kind === 'tool' && f.callId === 'completion-3')).toMatchObject({ name: 'write_stdin', mutating: false });
    expect(match([...calls, completed, { code: 0 }, completed]).progress.done).toBe(1);
  });

  test('a read-only process completion preserves a mid-flight check', () => {
    expect(match([
      { cmd: 'cat input.txt', result: { session_id: 91, output: 'running' } },
      { code: 0 },
      { name: 'write_stdin', input: { session_id: 91, chars: '' }, code: 0, output: '' },
    ]).progress.done).toBe(1);
  });

  test.each([
    'cat input.txt # note\nprintf changed > src/widget.ts',
    'cat input.txt # note\r\nprintf changed > src/widget.ts',
    'cat input.txt # note\n# another comment\nprintf changed > src/widget.ts',
  ])('a shell command after a comment newline invalidates prior checks: %s', cmd => {
    expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).items[0].status).toBe('unverified');
  });

  test.each(['cat input.txt # note', 'cat input.txt # note\n   ', "cat '# literal'"])(
    'a trailing comment or quoted marker remains read-only: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).progress.done).toBe(1);
    });
  test.each([
    "grep 'foo|bar' file.txt",
    "printf '%s' 'bun test'",
    "printf '%s\\n' '$HOME'",
    'printf "%s\\n" "a|b"',
    'printf "%s" "\\$HOME"',
    "grep foo\\|bar file.txt",
  ])('quoted or escaped shell data permits an exact receipt: %s', (cmd) => {
    const items = decompose([], 'quoted-receipt', [{ id: 'check', text: `Run \`${cmd}\`` }]);
    expect(items[0].anchors.commands).toEqual([cmd]);
    expect(matchLedger(recordedCalls([{ cmd, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(1);
    expect(matchLedger(recordedCalls([{ cmd, code: 1, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(0);
  });

  test.each(['grep foo|cat', 'echo "$HOME"', 'echo "$(touch file.txt)"', 'echo `pwd`', 'echo (foo)', "printf '%s' 'unclosed"])(
    'shell control flow or expansion cannot prove a literal receipt: %s', (cmd) => {
      const items = decompose([], 'uncertain-receipt', [{ id: 'check', text: 'Run `bun test`' }]);
      items[0].anchors.commands = [cmd];
      expect(matchLedger(recordedCalls([{ cmd, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(0);
    });

  test.each(["sed -e 'w generated.ts' input.txt", "sed 's/a/b/w generated.ts' input.txt", "sed -f edits.sed input.txt", "sed -n -f '-i' widget.ts", "sed -n --file '-i' widget.ts"])(
    'sed scripts that write or are unknown invalidate prior checks: %s', (cmd) => {
      // -f names an external script, even when its filename resembles -i. Its contents are unknown.
      const stale = match([{ code: 0 }, { cmd, code: 0, output: '' }]);
      expect(stale.items[0].status).toBe('unverified');
      expect(stale.evidence.filter(e => e.kind === 'file')).toEqual([]);
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }, { code: 0 }]).progress.done).toBe(1);
    });

  test('a real sed write script invalidates checks without -i', () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-sed-write-'));
    try {
      writeFileSync(join(root, 'input.txt'), 'a\n');
      const cmd = "sed -e 'w generated.ts' input.txt";
      const result = spawnSync('sh', ['-c', cmd], { cwd: root, encoding: 'utf8' });
      if (result.error) throw result.error;
      expect(result.status).toBe(0);
      expect(readFileSync(join(root, 'generated.ts'), 'utf8')).toBe('a\n');
      expect(match([{ code: 0 }, { cmd, result: { exit_code: result.status, output: result.stdout + result.stderr } }]).progress.done).toBe(0);
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  test.each([0, 1])('Claude Bash supplies trusted exit evidence before extracting sed files: %s', (code) => {
    const parser = new TranscriptFacts('claude');
    const timestamp = '2026-10-08T01:00:00.000Z';
    parser.add({ type: 'assistant', timestamp, message: { content: [{ type: 'tool_use', id: 'sed', name: 'Bash', input: { command: "sed -i.bak 's/a/b/' src/private/a.ts" } }] } });
    parser.add({ type: 'user', timestamp, message: { content: [{ type: 'tool_result', tool_use_id: 'sed', content: '', is_error: code !== 0 }] } });
    const items = decompose([], 'claude-sed', [{ id: 'edit', text: 'Do not edit src/private/**' }]);
    const result = matchLedger(parser.facts, items, [], [], DEFAULT_LEDGER_CONFIG);
    expect(result.evidence.filter(e => e.kind === 'file').map(e => e.value)).toEqual(code === 0 ? ['src/private/a.ts'] : []);
    expect(result.trail.flatMap(t => t.violations)).toEqual(code === 0 ? ['Forbidden path: src/private/**'] : []);
  });

  test.each([0, undefined, 1])('a custom path writer respects success, pending and failure: %s', (code) => {
    const write = { name: 'mcp__filesystem__write_file', input: { path: 'src/widget.ts', content: 'updated' }, code, output: '' };
    expect(match([{ code: 0 }, write]).progress.done).toBe(code === 1 ? 1 : 0);
    expect(match([{ code: 0 }, write, { code: 0 }]).progress.done).toBe(1);
    expect(match([{ code: 0 }, { ...write, name: 'mcp__filesystem__read_file', code: 0 }]).progress.done).toBe(1);
  });

  test.each(['--help', '--version'])('GNU sed information hook receipts preserve option boundaries: %s', (option) => {
    const information = `sed ${option} -i.bak 's/a/b/' private.ts public.ts`;
    expect(match([{ code: 0 }, { cmd: information, code: 0, output: '' }]).progress.done).toBe(1);
    expect(recordedCalls([{ cmd: information, code: 0, output: '' }]).filter(f => f.kind === 'tool').flatMap(f => f.facts ?? []).filter(e => e.kind === 'file')).toEqual([]);
    const edit = `sed -i.bak 's/a/b/' -- ${option} public.ts`;
    const facts = recordedCalls([{ cmd: edit, code: 0, output: '' }]);
    expect(facts.filter(f => f.kind === 'tool').flatMap(f => f.facts ?? []).filter(e => e.kind === 'file').map(e => e.value)).toEqual([option, 'public.ts']);
  });

  for (const option of ['--help', '--version']) test.skipIf(!hasGnuSed)(`a real sed ${option} option does not invent file edits`, () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-sed-information-'));
    const cmd = `sed ${option} -i.bak 's/a/b/' private.ts public.ts`;
    try {
      for (const path of ['private.ts', 'public.ts']) writeFileSync(join(root, path), 'a\n');
      const result = spawnSync('sh', ['-c', cmd], { cwd: root, encoding: 'utf8' });
      if (result.error) throw result.error;
      expect(result.status).toBe(0);
      for (const path of ['private.ts', 'public.ts']) expect(readFileSync(join(root, path), 'utf8')).toBe('a\n');
      const facts = recordedCalls([{ cmd, result: { exit_code: result.status, output: result.stdout + result.stderr } }]);
      const items = decompose([], 'sed-information', [{ id: 'edit', text: 'Edit public.ts' }]);
      items[0].constraints = [{ kind: 'path_forbidden', value: 'private.ts' }];
      const matched = matchLedger(facts, items, [], [], DEFAULT_LEDGER_CONFIG);
      expect(matched.evidence.filter(e => e.kind === 'file')).toEqual([]);
      expect(matched.trail.flatMap(t => t.violations)).toEqual([]);
      expect(facts.find(fact => fact.kind === 'tool')?.mutating).toBe(false);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  for (const path of ['--help', '--version']) test.skipIf(!hasGnuSed)(`a real sed file named ${path} after -- retains edit evidence`, () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-sed-option-file-'));
    const cmd = `sed -i.bak 's/a/b/' -- ${path} public.ts`;
    try {
      for (const file of [path, 'public.ts']) writeFileSync(join(root, file), 'a\n');
      const result = spawnSync('sh', ['-c', cmd], { cwd: root, encoding: 'utf8' });
      if (result.error) throw result.error;
      expect(result.status).toBe(0);
      for (const file of [path, 'public.ts']) expect(readFileSync(join(root, file), 'utf8')).toBe('b\n');
      const facts = recordedCalls([{ cmd, result: { exit_code: result.status, output: result.stdout + result.stderr } }]);
      const items = decompose([], 'sed-option-file', [{ id: 'edit', text: 'Edit public.ts' }]);
      items[0].constraints = [{ kind: 'path_forbidden', value: path }];
      const matched = matchLedger(facts, items, [], [], DEFAULT_LEDGER_CONFIG);
      expect(matched.evidence.filter(e => e.kind === 'file').map(e => e.value)).toEqual([path, 'public.ts']);
      expect(matched.trail.flatMap(t => t.violations)).toEqual([`Forbidden path: ${path}`]);
      expect(facts.find(fact => fact.kind === 'tool')?.mutating).toBe(true);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test.each(['.', '..', '../..', 'src/..'])('plain command prose preserves a dot path operand: %s', (path) => {
    const items = decompose([], 'dot-command', [{ id: 'checkout', text: `Run git checkout ${path}` }]);
    expect(items[0].anchors.commands).toEqual([`git checkout ${path}`]);
    expect(matchLedger(recordedCalls([{ cmd: 'git checkout', code: 0 }]), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(0);
  });

  test('a successful checkout without a path cannot prove that the working tree was restored', () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-checkout-dot-'));
    const runGit = (...args: string[]) => {
      const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
      if (result.error) throw result.error;
      expect(result.status).toBe(0);
      return result;
    };
    try {
      runGit('init', '--quiet');
      writeFileSync(join(root, 'widget.ts'), 'before\n');
      runGit('add', 'widget.ts');
      runGit('-c', 'user.name=Ledger regression', '-c', 'user.email=ledger@example.invalid', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', 'commit', '--quiet', '-m', 'fixture');
      writeFileSync(join(root, 'widget.ts'), 'after\n');
      const bare = runGit('checkout');
      expect(readFileSync(join(root, 'widget.ts'), 'utf8')).toBe('after\n');
      const items = decompose([], 'real-checkout-dot', [{ id: 'checkout', text: 'Run git checkout .' }]);
      const calls: Call[] = [{ cmd: 'git checkout', result: { exit_code: bare.status, output: bare.stdout + bare.stderr } }];
      expect(matchLedger(recordedCalls(calls), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(0);
      const restored = runGit('checkout', '.');
      expect(readFileSync(join(root, 'widget.ts'), 'utf8')).toBe('before\n');
      calls.push({ cmd: 'git checkout .', result: { exit_code: restored.status, output: restored.stdout + restored.stderr } });
      expect(matchLedger(recordedCalls(calls), items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(1);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

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
    "sed -i.bak 's/a/b/' src/private/a.ts src/public/b.ts",
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

  test.each([
    "sed -i .bak 's@/src/private/a.ts@new@' public.ts",
    "sed -i 's@/src/private/a.ts@new@' public.ts",
  ])('ambiguous GNU/BSD bare -i invalidates checks without invented file facts: %s', (cmd) => {
    const items = decompose([], 'sed-ambiguous', [{ id: 'edit', text: 'Do not edit src/private/**' }]);
    const result = matchLedger(recordedCalls([{ cmd, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG);
    expect(result.evidence.filter(e => e.kind === 'file')).toEqual([]);
    expect(result.trail.flatMap(t => t.violations)).toEqual([]);
    expect(match([{ code: 0 }, { cmd, code: 0, output: '' }]).progress.done).toBe(0);
    expect(match([{ code: 0 }, { cmd, code: 0, output: '' }, { code: 0 }]).progress.done).toBe(1);
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

  test('a Git write criterion invalidates older checks before proving its own execution', () => {
    const checkout = 'git checkout -- src/widget.ts';
    const items = decompose([], 'git-write-criteria', [
      { id: 'tests', text: `Run \`${command}\`` },
      { id: 'restore', text: `Run \`${checkout}\`` },
    ]);
    const calls: Call[] = [{ code: 0 }, { cmd: checkout, code: 0, output: '' }];
    const stale = matchLedger(recordedCalls(calls), items, [], [], DEFAULT_LEDGER_CONFIG);
    expect(stale.items.map(item => item.status)).toEqual(['unverified', 'done']);
    expect(stale.items[0].evidenceIds).toEqual([]);
    expect(matchLedger(recordedCalls([...calls, { code: 0 }]), items, [], [], DEFAULT_LEDGER_CONFIG).items.map(item => item.status)).toEqual(['done', 'done']);
    expect(matchLedger(recordedCalls([...calls, { code: 0 }, { cmd: checkout, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG).items.map(item => item.status)).toEqual(['unverified', 'done']);
  });

  test.each([
    'CI=1 git checkout -- src/widget.ts',
    'CI=1 LANG=C /usr/bin/git checkout -- src/widget.ts',
    'env CI=1 git checkout -- src/widget.ts',
    '/usr/bin/env -- CI=1 /usr/bin/git checkout -- src/widget.ts',
    '/usr/bin/git checkout -- src/widget.ts',
    'CI=1 git --no-pager -C . checkout -- src/widget.ts',
  ])('a prefixed Git write criterion invalidates older checks: %s', checkout => {
    const items = decompose([], 'prefixed-git-write-criteria', [
      { id: 'tests', text: `Run \`${command}\`` },
      { id: 'restore', text: `Run \`${checkout}\`` },
    ]);
    const calls: Call[] = [{ code: 0 }, { cmd: checkout, code: 0, output: '' }];
    const stale = matchLedger(recordedCalls(calls), items, [], [], DEFAULT_LEDGER_CONFIG);
    expect(stale.items.map(item => item.status)).toEqual(['unverified', 'done']);
    expect(stale.items[0].evidenceIds).toEqual([]);
    const refreshed = [...calls, { code: 0 }];
    expect(matchLedger(recordedCalls(refreshed), items, [], [], DEFAULT_LEDGER_CONFIG).items.map(item => item.status)).toEqual(['done', 'done']);
    expect(matchLedger(recordedCalls([...refreshed, { cmd: checkout, code: 0, output: '' }]), items, [], [], DEFAULT_LEDGER_CONFIG).items.map(item => item.status)).toEqual(['unverified', 'done']);
  });

  test.each([
    'CI=1 git status --short',
    '/usr/bin/env -- CI=1 /usr/bin/git diff --check',
    '/usr/bin/git log -1',
    '/usr/bin/env -- CI=1 /usr/bin/git --no-pager -C "repo path" diff --check',
    'CI=1 git -Crepo status --short',
  ])('a prefixed read-only Git criterion preserves earlier checks: %s', cmd => {
    const facts = recordedCalls([{ code: 0 }, { cmd, code: 0, output: '' }]);
    const items = decompose([], 'prefixed-git-read-criteria', [
      { id: 'tests', text: `Run \`${command}\`` },
      { id: 'read', text: `Run \`${cmd}\`` },
    ]);
    expect(matchLedger(facts, items, [], [], DEFAULT_LEDGER_CONFIG).items.map(item => item.status)).toEqual(['done', 'done']);
  });

  test.each(['git log -1', 'git status --short', 'git diff --check', 'git -C repo status --short', 'git --no-pager log -1', 'git --no-pager -C "repo path" diff --check', 'git -Crepo status --short'])(
    'read-only Git criterion preserves older check receipts: %s', cmd => {
      expect(match([{ code: 0 }, { cmd, code: 0, output: '' }], [command, cmd]).progress.done).toBe(1);
    });

  test.each([
    { output: 'Commit message: Exit code: 1', is_error: false, expected: 0 },
    { output: 'Commit message: Exit code: 0', is_error: true, expected: 1 },
    { output: { exit_code: 1, output: 'Commit message: Exit code: 0' }, is_error: false, expected: 1 },
    { output: { exit_code: 0, output: 'Commit message: Exit code: 1' }, is_error: false, expected: 0 },
  ])('Claude host status outranks ordinary stdout exit phrases: %j', ({ output, is_error, expected }) => {
    const parser = new TranscriptFacts('claude');
    const timestamp = '2026-10-08T10:00:00.000Z';
    const cmd = 'git log -1 --format=%B';
    parser.add({ type: 'assistant', timestamp, message: { content: [{ type: 'tool_use', id: 'git-log', name: 'Bash', input: { command: cmd } }] } });
    parser.add({ type: 'user', timestamp, message: { content: [{ type: 'tool_result', tool_use_id: 'git-log', content: output, is_error }] } });
    const tool = parser.facts.find(f => f.kind === 'tool');
    expect(tool?.kind === 'tool' ? tool.exitCode : undefined).toBe(expected);
    const items = decompose([], 'claude-log-status', [{ id: 'log', text: `Run \`${cmd}\`` }]);
    expect(matchLedger(parser.facts, items, [], [], DEFAULT_LEDGER_CONFIG).progress.done).toBe(expected === 0 ? 1 : 0);
  });

  test.each([
    'cp fixture.ts src/widget.ts',
    'node write.js',
    'env CI=1 /usr/bin/node write.js',
    'npm run generate',
    './scripts/check',
    'env --unknown bun test',
  ])('a non-verification mutation criterion invalidates older checks: %s', cmd => {
    const items = decompose([], 'mutation-criteria', [
      { id: 'tests', text: `Run \`${command}\`` },
      { id: 'write', text: `Run \`${cmd}\`` },
    ]);
    const calls: Call[] = [{ code: 0 }, { cmd, code: 0, output: '' }];
    const stale = matchLedger(recordedCalls(calls), items, [], [], DEFAULT_LEDGER_CONFIG);
    expect(stale.items.map(item => item.status)).toEqual(['unverified', 'done']);
    expect(stale.items[0].evidenceIds).toEqual([]);
    expect(matchLedger(recordedCalls([...calls, { code: 0 }]), items, [], [], DEFAULT_LEDGER_CONFIG).items.map(item => item.status)).toEqual(['done', 'done']);
  });

  test.each([
    'bun test src/other.test.ts', 'npm test', 'pnpm run tests',
    'yarn run check', 'bun run typecheck', 'cargo test', 'cargo check',
    'pytest tests/widget.py', 'go test ./...',
    'CI=1 /usr/bin/env -- LANG=C /usr/local/bin/bun run typecheck',
  ])('a known direct verification criterion retains peer receipts: %s', cmd => {
    expect(match([{ code: 0 }, { cmd, code: 0 }], [command, cmd]).progress.done).toBe(1);
  });

  test.each(['exec_command', 'functions.exec_command', 'mcp__shell__exec_command'])(
    '%s supplies direct and asynchronous literal command receipts', name => {
      expect(match([{ name, code: 0 }]).progress.done).toBe(1);
      const start = { name, result: { session_id: 91, output: 'running' } };
      const poll = { name: 'write_stdin', input: { session_id: 91, chars: '' } };
      expect(match([start, { ...poll, code: 0 }]).progress.done).toBe(1);
      expect(match([start, { ...poll, code: 1 }]).progress.done).toBe(0);
    });

  test.each(['read_text_file', 'list_directory', 'mcp__filesystem__read_text_file', 'mcp__filesystem__list_directory'])(
    'path-bearing %s preserves successful checks', name => {
      expect(match([{ code: 0 }, { name, input: { path: 'src/widget.ts' }, code: 0, output: '1 pass' }]).progress.done).toBe(1);
    });

  test('read_and_write_file remains a path-bearing mutation', () => {
    expect(match([{ code: 0 }, { name: 'mcp__filesystem__read_and_write_file', input: { path: 'src/widget.ts' }, code: 0 }]).items[0].status).toBe('unverified');
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
    `sed -n --expression '-i' widget.ts`,
    `sed -n --expression=-i widget.ts`,
    `sed -n 's/x/y/p' widget.ts # -i`,
    `sed -n 's/new/write/p' widget.ts`,
    `sed -n '/new/p' widget.ts`,
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
