import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync, statSync, writeFileSync } from 'fs';
import { join } from 'path';
import { installClaudeStatusline, readClaudeContextUsage, uninstallClaudeStatusline } from '../adapters/claude/statusline.js';
import type { ClaudeSettings } from '../adapters/hook/types.js';
import { getKeeplineHome } from '../lib/paths.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { getLedger, ingestLedger, ledgerDetail, ledgerOverview } from '../services/ledger/service.js';
import { setupLedgerTest } from './ledger/helpers.js';

function payload(sessionId: string, input = 2, cached = 749598, capacity = 1000000) {
  return { session_id: sessionId, context_window: {
    context_window_size: capacity,
    current_usage: { input_tokens: input, cache_creation_input_tokens: 0, cache_read_input_tokens: cached, output_tokens: 99999 },
  } };
}
async function invoke(settings: ClaudeSettings, data: unknown) {
  const command = (settings.statusLine as { command: string }).command;
  const raw = typeof data === 'string' ? data : JSON.stringify(data);
  const child = Bun.spawn(['/bin/sh', '-c', command], { stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' });
  child.stdin.write(raw);
  child.stdin.end();
  const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
  return { code, stdout, stderr };
}

describe('Claude status-line context capture', () => {
  setupLedgerTest();

  test('captures the real window and input/cache usage while forwarding exact stdin and exit status', async () => {
    const original = { type: 'command', command: `python3 -c 'import sys; sys.stdout.buffer.write(sys.stdin.buffer.read()); sys.exit(7)'`, padding: 3, refreshInterval: 12 };
    const settings: ClaudeSettings = { statusLine: { ...original }, unrelated: true };
    expect(installClaudeStatusline(settings)).toBe(true);
    const raw = JSON.stringify({ ...payload('capture-exact-123'), note: '你好 $HOME `echo secret`' }) + '\n';
    expect(await invoke(settings, raw)).toEqual({ code: 7, stdout: raw, stderr: '' });
    expect(readClaudeContextUsage('capture-exact-123')).toMatchObject({ usedTokens: 749600, windowTokens: 1000000 });
    expect(statSync(join(getKeeplineHome(), 'context/claude-code/capture-exact-123.json')).mode & 0o777).toBe(0o600);
    expect(installClaudeStatusline(settings)).toBe(false);
    expect(uninstallClaudeStatusline(settings)).toBe(true);
    expect(settings).toEqual({ statusLine: original, unrelated: true });
    expect(uninstallClaudeStatusline(settings)).toBe(false);
  });

  test('supports no existing status line and preserves a later user replacement on uninstall', async () => {
    const settings: ClaudeSettings = {};
    installClaudeStatusline(settings);
    expect(await invoke(settings, payload('capture-empty-123'))).toEqual({ code: 0, stdout: '', stderr: '' });
    expect(uninstallClaudeStatusline(settings)).toBe(true);
    expect(settings.statusLine).toBeUndefined();
    installClaudeStatusline(settings);
    settings.statusLine = { type: 'command', command: 'echo new-user-status' };
    expect(uninstallClaudeStatusline(settings)).toBe(false);
    expect(settings.statusLine).toEqual({ type: 'command', command: 'echo new-user-status' });
  });

  test('collection failures cannot break the original status line or write outside the snapshot directory', async () => {
    const settings: ClaudeSettings = { statusLine: { type: 'command', command: 'cat' } };
    installClaudeStatusline(settings);
    for (const raw of ['not JSON', JSON.stringify(payload('../../escape'))]) {
      const result = await invoke(settings, raw);
      expect(result.code).toBe(0);
      expect(result.stdout).toBe(raw);
      expect(result.stderr).toContain('Keepline context capture failed:');
    }
    expect(existsSync(join(getKeeplineHome(), 'escape.json'))).toBe(false);
    // A filesystem error must also leave stdout and the original exit status alone.
    const brokenHome = join(getKeeplineHome(), 'blocked-context');
    const blocked: ClaudeSettings = { statusLine: { type: 'command', command: 'echo original-output' } };
    installClaudeStatusline(blocked, brokenHome);
    writeFileSync(join(brokenHome, 'context'), 'not a directory');
    const result = await invoke(blocked, payload('capture-blocked-123'));
    expect(result).toMatchObject({ code: 0, stdout: 'original-output\n' });
    expect(result.stderr).toContain('Keepline context capture failed:');
  });

  test('unknown or invalid telemetry never becomes a fabricated percentage; a real zero stays zero', async () => {
    const settings: ClaudeSettings = {};
    installClaudeStatusline(settings);
    const session = 'capture-validation-123';
    for (const contextWindow of [null, { context_window_size: 1000000, current_usage: null },
      { context_window_size: 0, current_usage: payload(session).context_window.current_usage },
      { context_window_size: 1000000, current_usage: { input_tokens: -1, cache_creation_input_tokens: 0, cache_read_input_tokens: 0 } },
      { context_window_size: 1000000, current_usage: { input_tokens: 1 } },
    ]) {
      expect((await invoke(settings, { session_id: session, context_window: contextWindow })).code).toBe(0);
      expect(readClaudeContextUsage(session)).toBeUndefined();
    }
    await invoke(settings, payload(session, 0, 0));
    expect(readClaudeContextUsage(session)).toMatchObject({ usedTokens: 0, windowTokens: 1000000 });
    expect(readClaudeContextUsage('never-captured-123')).toBeUndefined();
    const snapshot = join(getKeeplineHome(), 'context', 'claude-code', `${session}.json`);
    writeFileSync(snapshot, JSON.stringify({ sessionId: 'another-session', at: new Date().toISOString(), contextWindow: payload(session).context_window }));
    expect(readClaudeContextUsage(session)).toBeUndefined();
  });

  test('new snapshots and compaction update cached overview/details without rereading an unchanged transcript', async () => {
    const settings: ClaudeSettings = {};
    installClaudeStatusline(settings);
    const sessionId = 'capture-ledger-cache-123';
    const at = new Date().toISOString();
    const path = join(getKeeplineHome(), 'claude-context-transcript.jsonl');
    writeFileSync(path, JSON.stringify({ type: 'user', timestamp: at, uuid: 'turn-context', message: { content: 'Run widget checks' } }) + '\n');
    const session = sessionRepository.upsert({ sessionId, client: 'claude', directory: '/project', title: 'Context capture', initialPrompt: 'Run widget checks', lastActiveAt: new Date(), status: 'idle' });
    const parsed = { sessionId, directory: '/project', lastActiveAt: new Date(), messageCount: 1, toolCount: 0, sourcePath: path };
    await invoke(settings, payload(sessionId));
    expect((await ingestLedger(session, parsed))?.contextUsage).toMatchObject({ usedTokens: 749600, windowTokens: 1000000 });
    const transcriptBefore = readFileSync(path, 'utf8');
    await invoke(settings, payload(sessionId, 100, 4900, 200000));
    expect((await getLedger(sessionId))?.contextUsage).toMatchObject({ usedTokens: 5000, windowTokens: 200000 });
    expect((await ledgerOverview())[0].contextUsage).toMatchObject({ usedTokens: 5000, windowTokens: 200000 });
    expect((await ledgerDetail(sessionId))?.contextUsage).toMatchObject({ usedTokens: 5000, windowTokens: 200000 });
    await invoke(settings, { session_id: sessionId, context_window: { context_window_size: 200000, current_usage: null } });
    expect((await getLedger(sessionId))?.contextUsage).toBeUndefined();
    await invoke(settings, payload(sessionId, 1000, 9000, 200000));
    expect((await getLedger(sessionId))?.contextUsage).toMatchObject({ usedTokens: 10000, windowTokens: 200000 });
    expect(readFileSync(path, 'utf8')).toBe(transcriptBefore);
  });

  test('Desktop transcript usage reaches cached cards and a current reported capacity takes precedence', async () => {
    const sessionId = 'desktop-context-123';
    const timestamp = new Date(Date.now() - 60000).toISOString();
    const path = join(getKeeplineHome(), 'desktop-context.jsonl');
    writeFileSync(path, JSON.stringify({ type: 'assistant', timestamp, message: { model: 'claude-opus-5-5', content: [], usage: {
      input_tokens: 2, cache_creation_input_tokens: 802, cache_read_input_tokens: 84531, output_tokens: 831,
    } } }) + '\n');
    const session = sessionRepository.upsert({ sessionId, client: 'claude', directory: '/project', title: 'Desktop context', lastActiveAt: new Date(), status: 'idle' });
    const parsed = { sessionId, directory: '/project', lastActiveAt: new Date(), messageCount: 1, toolCount: 0, sourcePath: path };
    expect((await ingestLedger(session, parsed))?.contextUsage).toMatchObject({ usedTokens: 85335, windowTokens: 1000000 });
    expect((await getLedger(sessionId))?.contextUsage).toMatchObject({ usedTokens: 85335, windowTokens: 1000000 });
    expect((await ledgerOverview())[0].contextUsage).toMatchObject({ usedTokens: 85335, windowTokens: 1000000 });
    expect(await ingestLedger(session, parsed)).toBeNull();
    const settings: ClaudeSettings = {};
    installClaudeStatusline(settings);
    await invoke(settings, payload(sessionId, 2, 85333, 200000));
    expect((await getLedger(sessionId))?.contextUsage).toMatchObject({ usedTokens: 85335, windowTokens: 200000 });
    expect(readClaudeContextUsage(sessionId, { usedTokens: 90000, windowTokens: 1000000, at: new Date(Date.now() + 60000).toISOString() }))
      .toMatchObject({ usedTokens: 90000, windowTokens: 1000000 });
  });
});
