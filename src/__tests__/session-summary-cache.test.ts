import { afterEach, describe, expect, test } from 'bun:test';
import { Database } from 'bun:sqlite';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, appendFileSync, statSync, utimesSync } from 'fs';
import { tmpdir } from 'os';
import { join, resolve } from 'path';
import {
  cachedSessionSummary, closeSessionSummaryCache, SessionSummaryCacheError,
} from '../infrastructure/session-summary-cache.js';

const tempDirs: string[] = [];
afterEach(() => {
  closeSessionSummaryCache();
  for (const dir of tempDirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function tempDirectory(): string {
  const dir = mkdtempSync(join(tmpdir(), 'keepline-cache-fixture-'));
  tempDirs.push(dir);
  return dir;
}

function fixture() {
  const path = join(tempDirectory(), 'session.jsonl');
  writeFileSync(path, 'first');
  const summary = {
    sessionId: 'cached-session', directory: '/tmp/cache-project',
    firstMessage: 'Original task', lastMessage: 'Latest evidence',
    messageCount: 3, toolCount: 1,
    startedAt: new Date('2026-09-17T01:00:00Z'),
    lastActiveAt: new Date('2026-09-17T01:03:00Z'),
  };
  return { path, summary };
}

describe('persistent transcript summary cache', () => {
  test.each(['claude', 'codex'] as const)('%s scanners reuse summaries across restarts without losing details or updates', (runtime) => {
    const root = tempDirectory();
    const sessionsDir = runtime === 'claude' ? join(root, '.claude', 'projects') : join(root, 'sessions');
    const projectDir = runtime === 'claude' ? join(sessionsDir, 'project') : sessionsDir;
    mkdirSync(projectDir, { recursive: true });
    const id = '12345678-1234-1234-1234-123456789abc';
    const path = join(projectDir, runtime === 'codex' ? `rollout-${id}.jsonl` : `${id}.jsonl`);
    const timestamp = '2026-09-17T01:00:00Z';
    const message = (text: string) => runtime === 'claude'
      ? { type: 'user', uuid: text, sessionId: id, cwd: '/tmp/cache-project', timestamp,
          userType: 'external', message: { role: 'user', content: text } }
      : { type: 'response_item', timestamp,
          payload: { type: 'message', role: 'user', content: [{ type: 'input_text', text }] } };
    const events = runtime === 'claude' ? [message('Original task'),
      { type: 'assistant', uuid: 'assistant', sessionId: id, timestamp,
        message: { role: 'assistant', content: [{ type: 'tool_use', id: 'tool', name: 'Read', input: { file_path: '/tmp/example' } }] } },
    ] : [
      { type: 'session_meta', timestamp, payload: { id, cwd: '/tmp/cache-project' } }, message('Original task'),
      { type: 'response_item', timestamp, payload: { type: 'function_call', name: 'read_file', arguments: '{"path":"/tmp/example"}' } },
    ];
    writeFileSync(path, events.map((event) => JSON.stringify(event)).join('\n') + '\n');
    const scan = (details = false) => {
      const script = `
        const scanner = await import(${JSON.stringify(resolve(`src/adapters/${runtime}/scanner.ts`))});
        const cache = await import(${JSON.stringify(resolve('src/infrastructure/session-summary-cache.ts'))});
        const sessions = await scanner.${runtime === 'claude' ? 'getAllSessions' : 'getAllCodexSessions'}({
          ${runtime === 'codex' ? `sessionsDir: ${JSON.stringify(sessionsDir)},` : ''} includeToolCalls: ${details}
        });
        console.log(JSON.stringify({ sessions, cache: cache.sessionSummaryCacheStats() }));
        cache.closeSessionSummaryCache();
      `;
      const child = Bun.spawnSync([process.execPath, '-e', script], {
        env: { ...process.env, HOME: root, KEEPLINE_HOME: join(root, 'cache') }, stdout: 'pipe', stderr: 'pipe',
      });
      expect(child.exitCode).toBe(0);
      return JSON.parse(child.stdout.toString().trim().split('\n').at(-1)!);
    };
    const cold = scan();
    expect(cold.sessions).toHaveLength(1);
    expect(cold.cache).toEqual({ hits: 0, misses: 1, writes: 1 });
    const warm = scan();
    expect(warm.sessions).toEqual(cold.sessions);
    expect(warm.cache).toEqual({ hits: 1, misses: 0, writes: 0 });
    expect(warm.sessions[0].toolCalls).toBeUndefined();
    const details = scan(true);
    expect(details.sessions[0].toolCalls).toHaveLength(1);
    expect(details.cache).toEqual({ hits: 0, misses: 0, writes: 0 });
    const reply = runtime === 'claude'
      ? { type: 'assistant', uuid: 'new-reply', sessionId: id, timestamp,
          message: { role: 'assistant', content: [{ type: 'text', text: 'New agent reply' }] } }
      : { type: 'response_item', timestamp,
          payload: { type: 'message', role: 'assistant', content: [{ type: 'output_text', text: 'New agent reply' }] } };
    appendFileSync(path, JSON.stringify(message('New user request')) + '\n' + JSON.stringify(reply) + '\n');
    const changed = scan();
    expect(changed.cache).toEqual({ hits: 0, misses: 1, writes: 1 });
    expect(changed.sessions[0].firstMessage).toBe('New user request');
    expect(changed.sessions[0].lastMessage).toBe('New agent reply');
    // Claude counts external user turns; Codex counts both user and assistant messages.
    expect(changed.sessions[0].messageCount).toBe(cold.sessions[0].messageCount + (runtime === 'claude' ? 1 : 2));
  });

  test('reuses transcript facts in a new process with dates restored', async () => {
    const { path, summary } = fixture();
    expect(await cachedSessionSummary('codex', path, async () => summary)).toEqual(summary);
    closeSessionSummaryCache();
    const modulePath = resolve('src/infrastructure/session-summary-cache.ts');
    const child = Bun.spawn([process.execPath, '-e', `
      const { cachedSessionSummary, closeSessionSummaryCache } = await import(${JSON.stringify(modulePath)});
      const data = await cachedSessionSummary('codex', ${JSON.stringify(path)}, async () => {
        throw new Error('Unchanged transcript was parsed again');
      });
      if (!(data.startedAt instanceof Date) || !(data.lastActiveAt instanceof Date)) throw new Error('Dates lost');
      console.log(JSON.stringify(data));
      closeSessionSummaryCache();
    `], { stdout: 'pipe', stderr: 'pipe', env: process.env });
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited,
    ]);
    expect(stderr).toBe('');
    expect(code).toBe(0);
    expect(JSON.parse(stdout)).toEqual(JSON.parse(JSON.stringify(summary)));
  });

  test('invalidates same-size rewrites even if mtime is restored', async () => {
    const { path, summary } = fixture();
    const originalTime = new Date('2026-09-17T01:00:00Z');
    utimesSync(path, originalTime, originalTime);
    await cachedSessionSummary('claude', path, async () => summary);
    const before = statSync(path);
    await Bun.sleep(5);
    writeFileSync(path, 'other');
    utimesSync(path, before.atime, before.mtime);
    const after = statSync(path);
    expect(after.size).toBe(before.size);
    expect(after.mtimeMs).toBe(before.mtimeMs);
    expect(after.ctimeMs).not.toBe(before.ctimeMs);
    const updated = { ...summary, lastMessage: 'Changed evidence' };
    expect(await cachedSessionSummary('claude', path, async () => updated)).toEqual(updated);
  });

  test('does not cache a transcript that changes while being parsed', async () => {
    const { path, summary } = fixture();
    await cachedSessionSummary('codex', path, async () => {
      appendFileSync(path, '\nnew event');
      return summary;
    });
    let reparsed = false;
    await cachedSessionSummary('codex', path, async () => { reparsed = true; return summary; });
    expect(reparsed).toBe(true);
  });

  test('keeps parser failures uncached and separates runtime parsers', async () => {
    const { path, summary } = fixture();
    await expect(cachedSessionSummary('codex', path, async () => { throw new Error('bad transcript'); }))
      .rejects.toThrow('bad transcript');
    expect(await cachedSessionSummary('codex', path, async () => summary)).toEqual(summary);
    const other = { ...summary, sessionId: 'claude-session' };
    expect(await cachedSessionSummary('claude', path, async () => other)).toEqual(other);
  });

  test('reports corrupt cache data separately from transcript errors', async () => {
    const { path, summary } = fixture();
    await cachedSessionSummary('codex', path, async () => summary);
    closeSessionSummaryCache();
    const db = new Database(join(process.env.KEEPLINE_HOME!, 'session-summaries-v1.sqlite'));
    db.query('UPDATE summaries SET data = ? WHERE cache_key = ?').run('{bad', `codex:${path}`);
    db.close();
    await expect(cachedSessionSummary('codex', path, async () => summary))
      .rejects.toBeInstanceOf(SessionSummaryCacheError);
  });
});
