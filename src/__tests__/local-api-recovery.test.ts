import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { createLocalApiApp } from '../local-api/app.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, runSql } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { setupUser } from '../services/auth.service.js';
import { createServiceRecoveryHandler } from '../cli/service-recovery.js';
import {
  beginSessionReconciliation, invalidateSessionClaims, isSessionReconciliationRunning,
} from '../services/session-reconciliation-gate.js';
import {
  createRecoveryProcessRunner,
  type LocalRecoveryPreview,
} from '../local-api/routes/recovery.js';

const preview: LocalRecoveryPreview = {
  sessionId: 'recover-session-1234',
  runtimeId: 'codex',
  method: 'resume',
  executable: 'codex',
  arguments: ['resume', '019ed4a3-2186-7e51-9aa1-ca1e376549b8'],
  directory: process.cwd(),
  createsNewSession: false,
  confirmationId: 'a'.repeat(64),
};

describe('Local API recovery confirmation', () => {
  beforeEach(() => resetDatabase());
  afterEach(() => closeDatabase());

  test('blocks preview and execution while a peer has invalidated live claims', async () => {
    let calls = 0;
    const app = createLocalApiApp({ recoveryRunner: async () => {
      calls++;
      return { preview, executed: true };
    } });
    const { token } = await setupUser('peer-gate-recovery-user', 'password123');
    invalidateSessionClaims(beginSessionReconciliation('daemon'));
    const headers = { Authorization: `Bearer ${token}`, 'content-type': 'application/json' };
    for (const [path, method, body] of [
      ['recovery-preview', 'GET', undefined],
      ['recover', 'POST', JSON.stringify({
        confirmationId: preview.confirmationId, terminalApp: 'auto',
        idempotencyKey: 'peer-gate-recovery-request',
      })],
    ] as const) {
      const response = await app.fetch(new Request(
        `http://localhost/api/v1/sessions/${preview.sessionId}/${path}`, { method, headers, body }
      ));
      expect(response.status).toBe(503);
      expect(await response.json()).toEqual({
        success: false, error: 'Startup reconciliation is still running',
      });
    }
    expect(calls).toBe(0);
  });

  test('isolated execution rechecks a gate acquired after confirmation preview', () => {
    let opened = false;
    let markedRunning = false;
    const handler = createServiceRecoveryHandler({
      recoverySource: () => ({
        sessionId: preview.sessionId, runtimeId: 'codex', directory: preview.directory,
        status: 'lost', availableMethods: ['resume'], recommendedMethod: 'resume',
      }),
      openTerminal: () => { opened = true; },
      markRunning: () => { markedRunning = true; },
    });
    const confirmed = handler.preview(preview.sessionId);
    invalidateSessionClaims(beginSessionReconciliation('daemon'));
    expect(() => handler.execute(preview.sessionId, confirmed.confirmationId, 'auto')).toThrow(
      'Startup reconciliation is still running'
    );
    expect(opened).toBe(false);
    expect(markedRunning).toBe(false);
  });

  test('production recovery helper reads the same blocked gate in its child process', async () => {
    const directory = mkdtempSync(join(tmpdir(), 'keepline-gated-helper-'));
    const script = join(directory, 'helper.ts');
    writeFileSync(script, `
      import { serviceRecoveryCommand } from ${JSON.stringify(new URL('../cli/service-recovery.ts', import.meta.url).href)};
      await serviceRecoveryCommand(process.argv.slice(2));
    `);
    invalidateSessionClaims(beginSessionReconciliation('daemon'));
    const runner = createRecoveryProcessRunner([process.execPath, script]);
    await expect(runner({ action: 'preview', sessionId: preview.sessionId })).rejects.toMatchObject({
      status: 503, message: 'Startup reconciliation is still running',
    });
    await expect(runner({
      action: 'execute', sessionId: preview.sessionId,
      confirmationId: preview.confirmationId, terminalApp: 'auto',
    })).rejects.toMatchObject({ status: 503, message: 'Startup reconciliation is still running' });
  });

  test('stalled terminal automation releases the writer and blocks uncertain-launch retries', async () => {
    sessionRepository.upsert({
      sessionId: preview.sessionId, client: 'codex', directory: preview.directory, status: 'lost',
    });
    const directory = mkdtempSync(join(tmpdir(), 'keepline-stalled-automation-'));
    const osascript = join(directory, 'osascript');
    const launches = join(directory, 'launches');
    writeFileSync(osascript, '#!/bin/sh\ncase "$2" in *"System Events"*) printf false; exit 0;; esac\nprintf "launched\\n" >> "$KEEPLINE_TEST_LAUNCH_RECORD"\nexec /bin/sleep 6\n');
    chmodSync(osascript, 0o755);
    const child = Bun.spawn([process.execPath, '-e', `
      import { createServiceRecoveryHandler } from './src/cli/service-recovery.ts';
      import { openTerminalWithArgv } from './src/services/terminal.ts';
      import { sessionRepository } from './src/infrastructure/database/repositories/session.repository.ts';
      const sessionId = ${JSON.stringify(preview.sessionId)};
      const handler = createServiceRecoveryHandler({
        recoverySource: () => ({ sessionId, runtimeId: 'codex', status: 'lost',
          directory: ${JSON.stringify(preview.directory)}, availableMethods: ['resume'], recommendedMethod: 'resume' }),
        openTerminal: (...args) => { console.log('automation-start'); openTerminalWithArgv(...args); },
        markRunning: () => sessionRepository.upsert({ sessionId, status: 'running' }),
      });
      const confirmed = handler.preview(sessionId);
      try { handler.execute(sessionId, confirmed.confirmationId, 'auto'); }
      catch { process.exitCode = 1; }
    `], {
      env: { ...process.env, PATH: directory + ':' + process.env.PATH,
        KEEPLINE_TEST_LAUNCH_RECORD: launches }, stdout: 'pipe', stderr: 'pipe',
    });
    try {
      const output = await child.stdout.getReader().read();
      expect(new TextDecoder().decode(output.value)).toContain('automation-start');
      const started = Date.now();
      runSql("INSERT INTO metadata (key, value) VALUES ('peer-writer-probe', 'unblocked')");
      expect(Date.now() - started).toBeLessThan(4_500);
      expect(await child.exited).toBe(1);
      expect(sessionRepository.findBySessionId(preview.sessionId)?.status).toBe('lost');
      expect(readFileSync(launches, 'utf8')).toBe('launched\n');
      expect(isSessionReconciliationRunning()).toBe(true);
      const retry = createServiceRecoveryHandler();
      expect(() => retry.preview(preview.sessionId)).toThrow('Startup reconciliation is still running');
      expect(() => retry.execute(preview.sessionId, preview.confirmationId, 'auto')).toThrow(
        'Startup reconciliation is still running'
      );
      expect(readFileSync(launches, 'utf8')).toBe('launched\n');
    } finally {
      if (child.exitCode === null) child.kill('SIGKILL');
      await child.exited;
    }
  }, 10_000);

  test('ordinary terminal launch errors preserve the error and leave the gate clear', () => {
    let markedRunning = false;
    const handler = createServiceRecoveryHandler({
      recoverySource: () => ({
        sessionId: preview.sessionId, runtimeId: 'codex', directory: preview.directory,
        status: 'lost', availableMethods: ['resume'], recommendedMethod: 'resume',
      }),
      openTerminal: () => { throw Object.assign(new Error('Terminal unavailable'), { code: 'ENOENT' }); },
      markRunning: () => { markedRunning = true; },
    });
    const confirmed = handler.preview(preview.sessionId);
    expect(() => handler.execute(preview.sessionId, confirmed.confirmationId, 'Terminal')).toThrow(
      'Terminal unavailable'
    );
    expect(markedRunning).toBe(false);
    expect(isSessionReconciliationRunning()).toBe(false);
  });

  test('previews once and executes an unchanged recovery idempotently', async () => {
    const requests: Array<{ action: string; sessionId: string }> = [];
    const app = createLocalApiApp({
      recoveryRunner: async (request) => {
        requests.push(request);
        return { preview, executed: request.action === 'execute' };
      },
    });
    const { token } = await setupUser('local-api-recovery-user', 'password123');
    sessionRepository.upsert({
      sessionId: preview.sessionId,
      client: 'codex',
      directory: preview.directory,
      status: 'lost',
    });
    const headers = { Authorization: `Bearer ${token}` };

    const metadata = await app.fetch(new Request('http://localhost/api/v1/meta'));
    const metadataBody = await metadata.json() as { data: { capabilities: string[] } };
    expect(metadataBody.data.capabilities).toContain('sessions.recovery.preview');
    expect(metadataBody.data.capabilities).toContain('sessions.recovery.execute');

    const previewResponse = await app.fetch(new Request(
      `http://localhost/api/v1/sessions/${preview.sessionId}/recovery-preview`,
      { headers }
    ));
    expect(previewResponse.status).toBe(200);
    expect(await previewResponse.json()).toMatchObject({
      success: true,
      data: { preview },
    });

    const body = JSON.stringify({
      confirmationId: preview.confirmationId,
      terminalApp: 'auto',
      idempotencyKey: 'recovery-request-1234',
    });
    const executeRequest = () => app.fetch(new Request(
      `http://localhost/api/v1/sessions/${preview.sessionId}/recover`,
      {
        method: 'POST',
        headers: { ...headers, 'content-type': 'application/json' },
        body,
      }
    ));
    expect((await executeRequest()).status).toBe(200);
    expect((await executeRequest()).status).toBe(200);
    expect(requests.map((request) => request.action)).toEqual(['preview', 'preview', 'execute']);

    const conflict = await app.fetch(new Request(
      `http://localhost/api/v1/sessions/${preview.sessionId}/recover`,
      {
        method: 'POST',
        headers: { ...headers, 'content-type': 'application/json' },
        body: JSON.stringify({
          confirmationId: 'b'.repeat(64),
          terminalApp: 'auto',
          idempotencyKey: 'recovery-request-1234',
        }),
      }
    ));
    expect(conflict.status).toBe(409);
  });
});

test('recovery process runner uses the structured helper protocol', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'keepline-recovery-runner-'));
  const script = join(directory, 'helper.ts');
  writeFileSync(script, `
    const [action, sessionId] = process.argv.slice(2);
    const preview = {
      sessionId,
      runtimeId: 'codex',
      method: 'resume',
      executable: 'codex',
      arguments: ['resume', sessionId],
      directory: ${JSON.stringify(process.cwd())},
      createsNewSession: false,
      confirmationId: '${'c'.repeat(64)}',
    };
    console.log('__KEEPLINE_SERVICE_RECOVERY__' + JSON.stringify({
      success: true,
      preview,
      executed: action === 'execute',
    }));
  `);
  const runner = createRecoveryProcessRunner([process.execPath, script]);
  const result = await runner({ action: 'preview', sessionId: 'runner-session-1234' });
  expect(result).toMatchObject({
    executed: false,
    preview: { sessionId: 'runner-session-1234', executable: 'codex' },
  });
});

test('isolated recovery helper executes only the preview the user confirmed', async () => {
  const opened: Array<{ executable: string; arguments: string[]; directory: string }> = [];
  let markedRunning = false;
  const handler = createServiceRecoveryHandler({
    recoverySource: () => ({
      sessionId: 'codex_019ed4a3-2186-7e51-9aa1-ca1e376549b8',
      runtimeId: 'codex',
      directory: process.cwd(),
      status: 'lost',
      initialPrompt: 'Continue safely',
      availableMethods: ['resume', 'continue', 'new'],
      recommendedMethod: 'resume',
    }),
    openTerminal: (executable, arguments_, directory) => {
      opened.push({ executable, arguments: arguments_, directory });
    },
    markRunning: () => { markedRunning = true; },
  });

  const result = handler.preview('codex_019ed4a3-2186-7e51-9aa1-ca1e376549b8');
  expect(result).toMatchObject({
    runtimeId: 'codex',
    method: 'resume',
    executable: 'codex',
    arguments: ['resume', '019ed4a3-2186-7e51-9aa1-ca1e376549b8'],
    directory: process.cwd(),
    createsNewSession: false,
  });
  expect(result.confirmationId).toMatch(/^[a-f0-9]{64}$/);
  expect(result.arguments).not.toContain('--dangerously-bypass-approvals-and-sandbox');

  expect(() => handler.execute(result.sessionId, 'b'.repeat(64), 'auto')).toThrow(
    'Recovery preview changed'
  );
  expect(opened).toHaveLength(0);

  const executed = handler.execute(result.sessionId, result.confirmationId, 'auto');
  expect(executed.executed).toBe(true);
  expect(opened).toEqual([{
    executable: 'codex',
    arguments: ['resume', '019ed4a3-2186-7e51-9aa1-ca1e376549b8'],
    directory: process.cwd(),
  }]);
  expect(markedRunning).toBe(true);
});
