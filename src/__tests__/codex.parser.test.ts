import { afterEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import {
  parseCodexSessionFile,
  scopeCodexSessionId,
  unscopeCodexSessionId,
} from '../adapters/codex/parser.js';

const tempDirs: string[] = [];

function createCodexJsonlFile(lines: Array<Record<string, unknown> | string>): string {
  const dir = mkdtempSync(join(tmpdir(), 'keepline-jsonl-'));
  tempDirs.push(dir);
  const filePath = join(dir, 'rollout-019ed4a3-2186-7e51-9aa1-ca1e376549b8.jsonl');
  const contents = lines
    .map((line) => (typeof line === 'string' ? line : JSON.stringify(line)))
    .join('\n');
  writeFileSync(filePath, `${contents}\n`);
  return filePath;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    rmSync(tempDirs.pop()!, { recursive: true, force: true });
  }
});

describe('Codex JSONL parser', () => {
  test('scopes session ids without changing resume ids', () => {
    const rawId = '019ed4a3-2186-7e51-9aa1-ca1e376549b8';
    expect(scopeCodexSessionId(rawId)).toBe(`codex_${rawId}`);
    expect(scopeCodexSessionId(`codex_${rawId}`)).toBe(`codex_${rawId}`);
    expect(unscopeCodexSessionId(`codex_${rawId}`)).toBe(rawId);
  });

  test('summarizes Codex messages and tool calls', async () => {
    const filePath = createCodexJsonlFile([
      {
        type: 'session_meta',
        timestamp: '2026-06-17T01:00:00.000Z',
        payload: {
          id: '019ed4a3-2186-7e51-9aa1-ca1e376549b8',
          cwd: '/tmp/codex-project',
        },
      },
      {
        type: 'response_item',
        timestamp: '2026-06-17T01:00:03.000Z',
        payload: {
          type: 'message',
          role: 'user',
          content: [{ type: 'input_text', text: 'Add Codex detection' }],
        },
      },
      {
        type: 'response_item',
        timestamp: '2026-06-17T01:00:05.000Z',
        payload: {
          type: 'function_call',
          name: 'read_file',
          arguments: '{"path":"/tmp/codex-project/src/index.ts"}',
        },
      },
      {
        type: 'response_item',
        timestamp: '2026-06-17T01:00:09.000Z',
        payload: {
          type: 'message',
          role: 'assistant',
          content: [{ type: 'output_text', text: 'Detection added' }],
        },
      },
    ]);

    const parsed = await parseCodexSessionFile(filePath, { includeToolCalls: true });

    expect(parsed).not.toBeNull();
    expect(parsed!.client).toBe('codex');
    expect(parsed!.rawSessionId).toBe('019ed4a3-2186-7e51-9aa1-ca1e376549b8');
    expect(parsed!.sessionId).toBe('codex_019ed4a3-2186-7e51-9aa1-ca1e376549b8');
    expect(parsed!.directory).toBe('/tmp/codex-project');
    expect(parsed!.firstMessage).toBe('Add Codex detection');
    expect(parsed!.lastMessage).toBe('Detection added');
    expect(parsed!.messageCount).toBe(2);
    expect(parsed!.toolCount).toBe(1);
    expect(parsed!.lastTool).toBe('read_file');
    expect(parsed!.lastToolInput).toEqual({ path: '/tmp/codex-project/src/index.ts' });
    expect(parsed!.currentFile).toBe('/tmp/codex-project/src/index.ts');
    expect(parsed!.startedAt?.toISOString()).toBe('2026-06-17T01:00:00.000Z');
    expect(parsed!.lastActiveAt.toISOString()).toBe('2026-06-17T01:00:09.000Z');
    expect(parsed!.toolCalls).toEqual([
      {
        name: 'read_file',
        input: { path: '/tmp/codex-project/src/index.ts' },
        timestamp: '2026-06-17T01:00:05.000Z',
      },
    ]);
  });
});
