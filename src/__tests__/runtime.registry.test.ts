import { describe, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, realpathSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import type {
  AgentRuntimeAdapter,
  RuntimeDescriptor,
  RuntimeScanResult,
  RuntimeSession,
} from '../domain/runtime/index.js';
import { scopeCodexSessionId } from '../adapters/codex/parser.js';
import type { CodexParsedSessionData } from '../adapters/codex/types.js';
import type { ParsedSessionData } from '../domain/session/index.js';
import {
  ClaudeCodeRuntimeAdapter,
  CodexRuntimeAdapter,
  RuntimeRegistry,
  attributeRuntimeProcesses,
  createDefaultRuntimeRegistry,
} from '../adapters/runtimes/index.js';

function skipProcessAttribution(sessions: RuntimeSession[]) {
  return { sessions, errors: [] };
}

function runtimeSession(overrides: Partial<RuntimeSession> = {}): RuntimeSession {
  return {
    runtimeId: 'good-runtime',
    sessionId: 'safe-session-123',
    cwd: '/tmp/project',
    status: 'unknown',
    title: 'Runtime session',
    filesTouched: [],
    toolCount: 0,
    messageCount: 1,
    lastActiveAt: new Date('2026-01-01T00:00:00.000Z'),
    ...overrides,
  };
}

function stubDescriptor(overrides: Partial<RuntimeDescriptor> = {}): RuntimeDescriptor {
  return {
    id: 'good-runtime',
    displayName: 'Good Runtime',
    kind: 'cli',
    executableNames: ['good-runtime'],
    sessionPathHints: [],
    capabilities: ['session-history'],
    ...overrides,
  };
}

function stubAdapter(overrides: Partial<AgentRuntimeAdapter> = {}): AgentRuntimeAdapter {
  const descriptor = overrides.descriptor ?? stubDescriptor();
  return {
    descriptor,
    scanSessions: async (): Promise<RuntimeScanResult> => ({
      runtime: descriptor,
      sessions: [runtimeSession({ runtimeId: descriptor.id })],
      errors: [],
    }),
    ...overrides,
  };
}

describe('RuntimeRegistry', () => {
  test('default registry includes Claude Code and Codex adapters', () => {
    const ids = createDefaultRuntimeRegistry()
      .list()
      .map((adapter) => adapter.descriptor.id);

    expect(ids).toEqual(['claude-code', 'codex']);
  });

  test('rejects duplicate runtime IDs', () => {
    const descriptor = stubDescriptor({ id: 'dupe-runtime' });
    const registry = new RuntimeRegistry([stubAdapter({ descriptor })]);

    expect(() => registry.register(stubAdapter({ descriptor }))).toThrow(
      'Runtime adapter already registered: dupe-runtime'
    );
  });

  test('rejects resume capability without a recovery command builder', () => {
    expect(() =>
      new RuntimeRegistry([
        stubAdapter({
          descriptor: stubDescriptor({
            id: 'bad-resume-runtime',
            capabilities: ['session-history', 'resume'],
          }),
          buildRecoveryCommand: undefined,
        }),
      ])
    ).toThrow('declares resume without buildRecoveryCommand');
  });

  test('rejects feature capabilities without providers or compatibility routes', () => {
    expect(() =>
      new RuntimeRegistry([
        stubAdapter({
          descriptor: stubDescriptor({
            id: 'bad-quota-runtime',
            capabilities: ['session-history', 'quota'],
          }),
        }),
      ])
    ).toThrow('declares quota without a provider or compatibility route');
  });

  test('isolates scan failure from one adapter without hiding healthy sessions', async () => {
    const good = stubAdapter({
      descriptor: stubDescriptor({ id: 'healthy-runtime' }),
    });
    const badDescriptor = stubDescriptor({ id: 'broken-runtime' });
    const bad = stubAdapter({
      descriptor: badDescriptor,
      scanSessions: async () => {
        throw new Error('scan exploded');
      },
    });

    const results = await new RuntimeRegistry([good, bad]).scanAll();

    expect(results).toHaveLength(2);
    expect(results.find((result) => result.runtime.id === 'healthy-runtime')?.sessions).toHaveLength(1);
    expect(results.find((result) => result.runtime.id === 'broken-runtime')?.errors).toEqual([
      {
        runtimeId: 'broken-runtime',
        code: 'unknown',
        message: 'scan exploded',
        recoverable: true,
      },
    ]);
  });

  test('normalizes cwd roots and filters scans by projectRoot', async () => {
    const tmpRoot = mkdtempSync(join(tmpdir(), 'runtime-registry-'));
    try {
      const repoRoot = join(tmpRoot, 'repo');
      const repoChild = join(repoRoot, 'packages', 'app');
      const otherRoot = join(tmpRoot, 'other');
      mkdirSync(join(repoRoot, '.git'), { recursive: true });
      mkdirSync(repoChild, { recursive: true });
      mkdirSync(otherRoot, { recursive: true });

      const descriptor = stubDescriptor({ id: 'root-filter-runtime' });
      const adapter = stubAdapter({
        descriptor,
        scanSessions: async () => ({
          runtime: descriptor,
          sessions: [
            runtimeSession({
              runtimeId: descriptor.id,
              sessionId: 'repo-session',
              cwd: repoChild,
            }),
            runtimeSession({
              runtimeId: descriptor.id,
              sessionId: 'other-session',
              cwd: otherRoot,
            }),
          ],
          errors: [],
        }),
      });

      const result = await new RuntimeRegistry([adapter]).scanAll({ projectRoot: repoRoot });

      expect(result[0].sessions.map((session) => session.sessionId)).toEqual(['repo-session']);
      expect(result[0].sessions[0].projectRoot).toBe(realpathSync(repoRoot));
    } finally {
      rmSync(tmpRoot, { recursive: true, force: true });
    }
  });
});

describe('runtime process attribution', () => {
  test('decorates matched runtime sessions with live process state', () => {
    const now = new Date();
    const result = attributeRuntimeProcesses(
      'codex',
      [runtimeSession({
        runtimeId: 'codex',
        sessionId: 'codex-session',
        cwd: '/tmp/project',
        startedAt: now,
        lastActiveAt: now,
      })],
      () => [{
        client: 'codex',
        pid: 4242,
        cwd: '/tmp/project',
        tty: 'ttys001',
        cpu: 0.1,
        memory: 1.2,
        startTime: now,
        args: ['resume'],
      }]
    );

    expect(result.errors).toEqual([]);
    expect(result.sessions[0]).toMatchObject({
      pid: 4242,
      tty: 'ttys001',
      processRunning: true,
      cpuUsage: 0.1,
      memoryUsage: 1.2,
    });
    expect(result.sessions[0].status).not.toBe('unknown');
  });

  test('surfaces process scanner failures without dropping history sessions', () => {
    const result = attributeRuntimeProcesses(
      'claude-code',
      [runtimeSession({ runtimeId: 'claude-code' })],
      () => {
        throw new Error('ps failed');
      }
    );

    expect(result.sessions).toHaveLength(1);
    expect(result.errors).toEqual([
      {
        runtimeId: 'claude-code',
        code: 'unknown',
        message: 'ps failed',
        recoverable: true,
      },
    ]);
  });
});

describe('ClaudeCodeRuntimeAdapter', () => {
  test('maps parsed sessions without losing usage, sub-agent, and tool metadata', async () => {
    const startedAt = new Date('2026-01-01T00:00:00.000Z');
    const lastActiveAt = new Date('2026-01-01T00:05:00.000Z');
    const parsed: ParsedSessionData = {
      sessionId: 'claude-session-123',
      client: 'claude',
      directory: '/tmp/project',
      firstMessage: 'Implement runtime registry',
      lastMessage: 'Done',
      messageCount: 2,
      toolCount: 1,
      lastTool: 'Edit',
      lastToolInput: { path: '/tmp/project/src/index.ts' },
      currentFile: '/tmp/project/src/index.ts',
      startedAt,
      lastActiveAt,
      toolCalls: [{
        name: 'Edit',
        input: { path: '/tmp/project/src/index.ts' },
        timestamp: '2026-01-01T00:04:00.000Z',
      }],
      usageStats: {
        totalInputTokens: 10,
        totalOutputTokens: 20,
        totalTokens: 30,
        totalCost: 0.001,
        apiCalls: 1,
      },
      agentId: 'agent-123',
      parentSessionId: 'parent-session-123',
      isSubAgent: true,
    };

    const adapter = new ClaudeCodeRuntimeAdapter(async () => [parsed], skipProcessAttribution);
    const result = await adapter.scanSessions({ mode: 'full', includeSubAgents: true });
    const session = result.sessions[0];

    expect(result.errors).toEqual([]);
    expect(session).toMatchObject({
      runtimeId: 'claude-code',
      sessionId: 'claude-session-123',
      cwd: '/tmp/project',
      agentId: 'agent-123',
      parentRuntimeSessionId: 'parent-session-123',
      isSubAgent: true,
      usageStats: {
        totalInputTokens: 10,
        totalOutputTokens: 20,
        totalTokens: 30,
        totalCost: 0.001,
        apiCalls: 1,
      },
    });
    expect(session.parentSessionId).toBeUndefined();
    expect(session.filesTouched).toEqual(['/tmp/project/src/index.ts']);
    expect(session.toolCalls).toHaveLength(1);
  });

  test('builds structured recovery commands', () => {
    const adapter = new ClaudeCodeRuntimeAdapter(async () => []);
    const session = runtimeSession({
      runtimeId: 'claude-code',
      sessionId: 'claude-session-123',
      cwd: '/tmp/project',
      initialPrompt: 'Continue this work',
    });

    expect(adapter.buildRecoveryCommand?.(session, { method: 'resume' })).toEqual({
      executable: 'claude',
      args: ['--resume', 'claude-session-123'],
      cwd: '/tmp/project',
    });
    expect(adapter.buildRecoveryCommand?.(session, {
      method: 'continue',
      skipPermissions: true,
    })).toEqual({
      executable: 'claude',
      args: ['--dangerously-skip-permissions', '--continue'],
      cwd: '/tmp/project',
    });
    expect(adapter.buildRecoveryCommand?.(session, { method: 'new' })).toEqual({
      executable: 'claude',
      args: ['Continue this work'],
      cwd: '/tmp/project',
    });
  });

  test('returns structured scan errors when the legacy scanner fails', async () => {
    const adapter = new ClaudeCodeRuntimeAdapter(async () => {
      throw new Error('legacy scanner failed');
    });

    const result = await adapter.scanSessions();

    expect(result.sessions).toEqual([]);
    expect(result.errors).toEqual([
      {
        runtimeId: 'claude-code',
        code: 'unknown',
        message: 'legacy scanner failed',
        recoverable: true,
      },
    ]);
  });

  test('keeps healthy sessions visible when Claude per-file parsing fails', async () => {
    const parsed: ParsedSessionData = {
      sessionId: 'claude-session-123',
      directory: '/tmp/project',
      firstMessage: 'Healthy session',
      messageCount: 1,
      toolCount: 0,
      lastActiveAt: new Date('2026-01-01T00:00:00.000Z'),
    };
    const adapter = new ClaudeCodeRuntimeAdapter(async () => ({
      sessions: [parsed],
      failures: [{
        filePath: '/tmp/project/bad-session.jsonl',
        message: 'Invalid JSONL at line 3',
      }],
    }), skipProcessAttribution);

    const result = await adapter.scanSessions();

    expect(result.sessions).toHaveLength(1);
    expect(result.errors).toEqual([
      {
        runtimeId: 'claude-code',
        code: 'parse-failed',
        message: 'Invalid JSONL at line 3',
        sourcePath: '/tmp/project/bad-session.jsonl',
        recoverable: true,
      },
    ]);
  });
});

describe('CodexRuntimeAdapter', () => {
  test('maps parsed sessions to raw runtime session IDs', async () => {
    const rawSessionId = '019ed4a3-2186-7e51-9aa1-ca1e376549b8';
    const parsed: CodexParsedSessionData = {
      client: 'codex',
      rawSessionId,
      sessionId: scopeCodexSessionId(rawSessionId),
      directory: '/tmp/project',
      firstMessage: 'Codex task',
      lastMessage: 'Codex done',
      messageCount: 2,
      toolCount: 0,
      startedAt: new Date('2026-01-01T00:00:00.000Z'),
      lastActiveAt: new Date('2026-01-01T00:05:00.000Z'),
    };

    const adapter = new CodexRuntimeAdapter(async () => [parsed], skipProcessAttribution);
    const result = await adapter.scanSessions();

    expect(result.sessions[0]).toMatchObject({
      runtimeId: 'codex',
      sessionId: rawSessionId,
      cwd: '/tmp/project',
      runtimeMetadata: {
        legacyClient: 'codex',
        scopedSessionId: scopeCodexSessionId(rawSessionId),
      },
    });
  });

  test('builds structured recovery commands with raw session IDs', () => {
    const rawSessionId = '019ed4a3-2186-7e51-9aa1-ca1e376549b8';
    const adapter = new CodexRuntimeAdapter(async () => []);
    const session = runtimeSession({
      runtimeId: 'codex',
      sessionId: rawSessionId,
      cwd: '/tmp/project',
      initialPrompt: 'Start codex work',
    });

    expect(adapter.buildRecoveryCommand?.(session, { method: 'resume' })).toEqual({
      executable: 'codex',
      args: ['resume', rawSessionId],
      cwd: '/tmp/project',
    });
    expect(adapter.buildRecoveryCommand?.(session, {
      method: 'continue',
      skipPermissions: true,
    })).toEqual({
      executable: 'codex',
      args: ['resume', '--dangerously-bypass-approvals-and-sandbox', '--last'],
      cwd: '/tmp/project',
    });
    expect(adapter.buildRecoveryCommand?.(session, { method: 'new' })).toEqual({
      executable: 'codex',
      args: ['Start codex work'],
      cwd: '/tmp/project',
    });
  });

  test('keeps healthy sessions visible when Codex per-file parsing fails', async () => {
    const rawSessionId = '019ed4a3-2186-7e51-9aa1-ca1e376549b8';
    const parsed: CodexParsedSessionData = {
      client: 'codex',
      rawSessionId,
      sessionId: scopeCodexSessionId(rawSessionId),
      directory: '/tmp/project',
      firstMessage: 'Healthy Codex session',
      messageCount: 1,
      toolCount: 0,
      lastActiveAt: new Date('2026-01-01T00:00:00.000Z'),
    };
    const adapter = new CodexRuntimeAdapter(async () => ({
      sessions: [parsed],
      failures: [{
        filePath: '/tmp/codex/bad-rollout.jsonl',
        message: 'Invalid JSONL at line 7',
      }],
    }), skipProcessAttribution);

    const result = await adapter.scanSessions();

    expect(result.sessions).toHaveLength(1);
    expect(result.errors).toEqual([
      {
        runtimeId: 'codex',
        code: 'parse-failed',
        message: 'Invalid JSONL at line 7',
        sourcePath: '/tmp/codex/bad-rollout.jsonl',
        recoverable: true,
      },
    ]);
  });
});
