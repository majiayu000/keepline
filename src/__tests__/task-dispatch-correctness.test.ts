import { afterEach, beforeEach, describe, expect, spyOn, test } from 'bun:test';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import * as codexScanner from '../adapters/codex/scanner.js';
import { createLocalApiApp } from '../local-api/app.js';
import { resetDatabase } from '../db/migrations.js';
import { encodeAgentSessionId } from '../domain/work-item/index.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { taskDispatchRepository } from '../infrastructure/database/repositories/task-dispatch.repository.js';
import { getDatabase } from '../infrastructure/database/sqlite.js';
import { workItemEvidenceRepository } from '../infrastructure/database/repositories/work-item-evidence.repository.js';
import { workItemRepository } from '../infrastructure/database/repositories/work-item.repository.js';
import { setupUser } from '../services/auth.service.js';
import {
  DISPATCH_CORRELATION_TIMEOUT_ERROR,
  DispatchSessionClaimConflictError,
  DispatchIdempotencyConflictError,
  TaskDispatchService,
} from '../services/task-dispatch.service.js';

describe('task dispatch correctness', () => {
  beforeEach(() => resetDatabase());

  test('gives launched agents a dispatch marker without changing stored prompts', async () => {
    const launches: Array<{ executable: string; args: string[] }> = [];
    const service = new TaskDispatchService({
      launch: (executable, args) => { launches.push({ executable, args }); },
    });
    const claudeItem = workItemRepository.create({ title: 'Claude claim' });
    const codexItem = workItemRepository.create({ title: 'Codex manual completion' });

    const claudeDispatch = await service.dispatch(claudeItem.id, {
      runtimeId: 'claude-code', cwd: '/tmp', prompt: 'Run the checks', idempotencyKey: 'claim-contract',
    });
    await service.dispatch(codexItem.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Run Codex checks', idempotencyKey: 'codex-contract',
    });

    expect(claudeDispatch.prompt).toBe('Run the checks');
    expect(launches[0]).toEqual({
      executable: 'claude',
      args: [
        `Run the checks\n\nKEEPLINE_DISPATCH_ID:${claudeDispatch.id}\n\nOnly after the task is fully complete and verified, end your final response with this exact line:\nKEEPLINE_COMPLETE_WORK_ITEM:${claudeItem.id}\nDo not output that line when blocked, waiting for input, or incomplete.`,
      ],
    });
    const codexDispatch = taskDispatchRepository.findByIdempotencyKey('codex-contract')!;
    expect(launches[1]).toEqual({
      executable: 'codex',
      args: [`Run Codex checks\n\nKEEPLINE_DISPATCH_ID:${codexDispatch.id}`],
    });
  });

  test('does not auto-link an unrelated sole session from the same directory', async () => {
    const now = new Date('2026-08-30T00:00:00.000Z');
    const launches: Array<{ args: string[] }> = [];
    const service = new TaskDispatchService({
      now: () => now,
      launch: (_executable, args) => { launches.push({ args }); },
    });
    const item = workItemRepository.create({ title: 'Trusted dispatch' });
    const dispatch = await service.dispatch(item.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Trusted prompt', idempotencyKey: 'trusted-dispatch',
    });
    sessionRepository.upsert({
      sessionId: 'codex_unrelated-session', client: 'codex', directory: '/tmp',
      initialPrompt: 'An unrelated prompt', title: 'Unrelated', status: 'running',
      lastActiveAt: new Date(now.getTime() + 1),
    });

    await service.reconcilePending();

    expect(taskDispatchRepository.findById(dispatch.id)?.state).toBe('ambiguous');
    expect(taskDispatchRepository.findById(dispatch.id)?.linkedAgentSessionId).toBeUndefined();

    sessionRepository.upsert({
      sessionId: 'codex_trusted-session', client: 'codex', directory: '/tmp',
      initialPrompt: launches[0].args[0], title: 'Trusted', status: 'running',
      lastActiveAt: new Date(now.getTime() + 2),
    });
    await service.reconcilePending();

    expect(taskDispatchRepository.findById(dispatch.id)?.state).toBe('linked');
    expect(taskDispatchRepository.findById(dispatch.id)?.linkedAgentSessionId)
      .toBe(encodeAgentSessionId('codex', 'codex_trusted-session'));
  });

  test('refreshes ambiguous candidates so a later session can be resolved through the API', async () => {
    const now = new Date('2026-08-30T00:00:00.000Z');
    const service = new TaskDispatchService({ now: () => now, launch: () => {} });
    const item = workItemRepository.create({ title: 'Refresh candidates' });
    const dispatch = await service.dispatch(item.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Refresh', idempotencyKey: 'refresh-candidates',
    });
    sessionRepository.upsert({
      sessionId: 'codex_first-candidate', client: 'codex', directory: '/tmp',
      initialPrompt: 'Unmarked first', title: 'First', status: 'running',
      lastActiveAt: new Date(now.getTime() + 1),
    });
    await service.reconcilePending();
    expect(taskDispatchRepository.findById(dispatch.id)?.candidateSessionIds)
      .toEqual(['codex_first-candidate']);
    expect(taskDispatchRepository.findCorrelationPending().map((row) => row.id))
      .toContain(dispatch.id);

    sessionRepository.upsert({
      sessionId: 'codex_first-candidate', client: 'codex', directory: '/tmp',
      lastActiveAt: new Date(now.getTime() - 1),
    });
    sessionRepository.upsert({
      sessionId: 'codex_later-candidate', client: 'codex', directory: '/tmp',
      initialPrompt: 'Unmarked later', title: 'Later', status: 'running',
      lastActiveAt: new Date(now.getTime() + 2),
    });
    await service.reconcilePending();
    expect(taskDispatchRepository.findById(dispatch.id)?.candidateSessionIds)
      .toEqual(['codex_later-candidate']);

    const { token } = await setupUser('late-candidate-user', 'password123');
    const response = await createLocalApiApp().fetch(new Request(
      `http://localhost/api/v1/dispatches/${dispatch.id}/resolve-session`,
      {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ sessionId: 'codex_later-candidate' }),
      }
    ));
    expect(response.status).toBe(200);
    expect(taskDispatchRepository.findById(dispatch.id)?.linkedAgentSessionId)
      .toBe(encodeAgentSessionId('codex', 'codex_later-candidate'));
  });

  test('fails an ambiguous dispatch at its deadline and stops correlation scans', async () => {
    let now = new Date('2026-08-30T00:00:00.000Z');
    const service = new TaskDispatchService({
      now: () => now, correlationTimeoutMs: 1_000, launch: () => {},
    });
    const item = workItemRepository.create({ title: 'Ambiguous deadline' });
    const dispatch = await service.dispatch(item.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Wait', idempotencyKey: 'ambiguous-deadline',
    });
    sessionRepository.upsert({
      sessionId: 'codex_unmarked-deadline', client: 'codex', directory: '/tmp',
      initialPrompt: 'Unmarked', title: 'Unmarked', status: 'running',
      lastActiveAt: new Date(now.getTime() + 1),
    });
    await service.reconcilePending();
    expect(taskDispatchRepository.findById(dispatch.id)?.state).toBe('ambiguous');
    now = new Date(now.getTime() + 1_001);
    await service.reconcilePending();
    expect(taskDispatchRepository.findById(dispatch.id)?.state).toBe('failed');
    expect(taskDispatchRepository.findById(dispatch.id)?.error)
      .toBe(DISPATCH_CORRELATION_TIMEOUT_ERROR);
    expect(taskDispatchRepository.findCorrelationPending()).toEqual([]);
  });

  test('makes every dispatch ambiguous when multiple tasks match the same new session', async () => {
    const now = new Date('2026-08-30T00:00:00.000Z');
    const service = new TaskDispatchService({ now: () => now, launch: () => {} });
    const first = workItemRepository.create({ title: 'First task' });
    const second = workItemRepository.create({ title: 'Second task' });
    const firstDispatch = await service.dispatch(first.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'First prompt', idempotencyKey: 'shared-first',
    });
    const secondDispatch = await service.dispatch(second.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Second prompt', idempotencyKey: 'shared-second',
    });
    sessionRepository.upsert({
      sessionId: 'codex_shared-new-session',
      client: 'codex',
      directory: '/tmp',
      initialPrompt: 'Shared result',
      title: 'Shared result',
      status: 'running',
      lastActiveAt: new Date(now.getTime() + 1),
    });

    await service.reconcilePending();

    for (const id of [firstDispatch.id, secondDispatch.id]) {
      const dispatch = taskDispatchRepository.findById(id)!;
      expect(dispatch.state).toBe('ambiguous');
      expect(dispatch.candidateSessionIds).toEqual(['codex_shared-new-session']);
      expect(dispatch.linkedAgentSessionId).toBeUndefined();
    }
  });

  test('rejects a sequential second manual claim without creating another accepted link', async () => {
    const now = new Date('2026-08-30T00:00:00.000Z');
    const service = new TaskDispatchService({ now: () => now, launch: () => {} });
    const first = workItemRepository.create({ title: 'First claimant' });
    const second = workItemRepository.create({ title: 'Second claimant' });
    const firstDispatch = await service.dispatch(first.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'First', idempotencyKey: 'claim-first',
    });
    const secondDispatch = await service.dispatch(second.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Second', idempotencyKey: 'claim-second',
    });
    sessionRepository.upsert({
      sessionId: 'codex_manual-claim', client: 'codex', directory: '/tmp',
      title: 'Manual claim', status: 'running', lastActiveAt: new Date(now.getTime() + 1),
    });
    await service.reconcilePending();

    const linked = service.resolveSession(firstDispatch.id, 'codex_manual-claim');
    expect(linked.state).toBe('linked');
    expect(() => service.resolveSession(secondDispatch.id, 'codex_manual-claim'))
      .toThrow(DispatchSessionClaimConflictError);
    expect(taskDispatchRepository.findById(secondDispatch.id)?.state).toBe('ambiguous');
    expect(() => taskDispatchRepository.updateState(secondDispatch.id, 'linked', {
      linkedAgentSessionId: linked.linkedAgentSessionId,
    })).toThrow();
    expect(taskDispatchRepository.findById(secondDispatch.id)?.state).toBe('ambiguous');
    const links = getDatabase().prepare(`
      SELECT COUNT(*) AS count FROM work_item_session_links
      WHERE agent_session_id = ? AND acceptance_status = 'accepted'
    `).get(linked.linkedAgentSessionId!) as { count: number };
    expect(links.count).toBe(1);
  });

  test('returns one success and one 409 when manual resolve requests race', async () => {
    const now = new Date('2026-08-30T00:00:00.000Z');
    const service = new TaskDispatchService({ now: () => now, launch: () => {} });
    const first = workItemRepository.create({ title: 'Race first' });
    const second = workItemRepository.create({ title: 'Race second' });
    const dispatches = await Promise.all([
      service.dispatch(first.id, {
        runtimeId: 'codex', cwd: '/tmp', prompt: 'Race first', idempotencyKey: 'race-first',
      }),
      service.dispatch(second.id, {
        runtimeId: 'codex', cwd: '/tmp', prompt: 'Race second', idempotencyKey: 'race-second',
      }),
    ]);
    sessionRepository.upsert({
      sessionId: 'codex_racing-claim', client: 'codex', directory: '/tmp',
      title: 'Racing claim', status: 'running', lastActiveAt: new Date(now.getTime() + 1),
    });
    await service.reconcilePending();
    const { token } = await setupUser('dispatch-race-user', 'password123');
    const app = createLocalApiApp();
    const responses = await Promise.all(dispatches.map((dispatch) => app.fetch(new Request(
      `http://localhost/api/v1/dispatches/${dispatch.id}/resolve-session`,
      {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ sessionId: 'codex_racing-claim' }),
      }
    ))));
    expect(responses.map((response) => response.status).sort()).toEqual([200, 409]);
    const agentSessionId = encodeAgentSessionId('codex', 'codex_racing-claim');
    const claimed = taskDispatchRepository.findLinkedByAgentSessionId(agentSessionId);
    expect(claimed).toHaveLength(1);
    const acceptedLinks = getDatabase().prepare(`
      SELECT COUNT(*) AS count FROM work_item_session_links
      WHERE agent_session_id = ? AND acceptance_status = 'accepted'
    `).get(agentSessionId) as { count: number };
    expect(acceptedLinks.count).toBe(1);
  });

  test('preserves a manual resolution committed while correlation reads a transcript', async () => {
    const now = new Date('2026-08-30T00:00:00.000Z');
    const service = new TaskDispatchService({ now: () => now, launch: () => {} });
    const item = workItemRepository.create({ title: 'Concurrent resolution' });
    const dispatch = await service.dispatch(item.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Resolve', idempotencyKey: 'resolve-during-read',
    });
    sessionRepository.upsert({
      sessionId: 'codex_resolve-during-read', client: 'codex', directory: '/tmp',
      initialPrompt: 'Unmarked candidate', title: 'Candidate', status: 'running',
      lastActiveAt: new Date(now.getTime() + 1),
    });
    await service.reconcilePending();
    let release!: () => void;
    const blocked = new Promise<void>((resolve) => { release = resolve; });
    let reading!: () => void;
    const started = new Promise<void>((resolve) => { reading = resolve; });
    const read = spyOn(codexScanner, 'getCodexSessionById').mockImplementation(async () => {
      reading();
      await blocked;
      return null;
    });
    try {
      const scan = service.reconcilePending();
      await started;
      service.resolveSession(dispatch.id, 'codex_resolve-during-read');
      release();
      await scan;
      expect(taskDispatchRepository.findById(dispatch.id)?.state).toBe('linked');
    } finally {
      release();
      read.mockRestore();
    }
  });

  test('reuses only a canonical idempotent payload and returns 409 for conflicts', async () => {
    let launches = 0;
    const service = new TaskDispatchService({ launch: () => { launches++; } });
    const item = workItemRepository.create({ title: 'Canonical payload' });
    const input = {
      runtimeId: 'codex' as const,
      cwd: '/tmp',
      prompt: 'Canonical prompt',
      terminalApp: 'auto' as const,
      idempotencyKey: 'canonical-key',
    };
    const first = await service.dispatch(item.id, input);
    const replay = await service.dispatch(item.id, { ...input, prompt: '  Canonical prompt  ' });
    expect(replay.id).toBe(first.id);
    expect(launches).toBe(1);
    await expect(service.dispatch(item.id, { ...input, prompt: 'Different prompt' }))
      .rejects.toBeInstanceOf(DispatchIdempotencyConflictError);

    const { token } = await setupUser('dispatch-conflict-user', 'password123');
    const response = await createLocalApiApp().fetch(new Request(
      `http://localhost/api/v1/work-items/${item.id}/dispatch`,
      {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...input, prompt: 'Route conflict' }),
      }
    ));
    expect(response.status).toBe(409);
  });

  test('fails an awaiting dispatch after its persisted deadline following restart', async () => {
    let now = new Date('2026-08-30T00:00:00.000Z');
    const item = workItemRepository.create({ title: 'Restart deadline' });
    const firstProcess = new TaskDispatchService({
      now: () => now,
      correlationTimeoutMs: 1_000,
      launch: () => {},
    });
    const created = await firstProcess.dispatch(item.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Wait for session', idempotencyKey: 'deadline-key',
    });
    expect(created.correlationDeadlineAt.toISOString()).toBe('2026-08-30T00:00:01.000Z');

    now = new Date('2026-08-30T00:00:01.001Z');
    const restartedProcess = new TaskDispatchService({ now: () => now, launch: () => {} });
    await restartedProcess.reconcilePending();
    const failed = taskDispatchRepository.findById(created.id)!;
    expect(failed.state).toBe('failed');
    expect(failed.error).toBe(DISPATCH_CORRELATION_TIMEOUT_ERROR);
  });

  test('keeps Stash task status authoritative until the next external upsert', async () => {
    const service = new TaskDispatchService({ launch: () => {} });
    const stashItem = workItemRepository.create({
      title: 'Stash truth',
      status: 'planned',
      externalSource: 'stash',
      externalId: 'stash-truth-1',
    });
    await service.dispatch(stashItem.id, {
      runtimeId: 'codex', cwd: '/tmp', prompt: 'Do work', idempotencyKey: 'stash-truth-dispatch',
    });
    expect(workItemRepository.findById(stashItem.id)?.status).toBe('planned');

    const agentSession = workItemEvidenceRepository.upsertAgentSession({
      runtimeId: 'codex', runtimeSessionId: 'codex_stash-truth-session', cwd: '/tmp',
      status: 'completed', title: 'Finished',
    });
    workItemEvidenceRepository.createSessionLink({
      workItemId: stashItem.id, agentSessionId: agentSession.id, linkSource: 'user',
    });
    const evidence = workItemEvidenceRepository.createProgressEvidence({
      agentSessionId: agentSession.id,
      kind: 'message', outcome: 'completed', confidence: 'explicit', summary: 'Explicitly finished',
    });
    const { token } = await setupUser('stash-truth-user', 'password123');
    const app = createLocalApiApp();
    const review = await app.fetch(new Request(
      `http://localhost/api/v1/work-items/${stashItem.id}/completion-review`,
      {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ evidenceId: evidence.id, decision: 'accepted' }),
      }
    ));
    expect(review.status).toBe(200);
    expect(workItemRepository.findById(stashItem.id)?.status).toBe('planned');

    const external = await app.fetch(new Request(
      'http://localhost/api/v1/work-items/external/stash/stash-truth-1',
      {
        method: 'PUT',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: 'Stash truth', status: 'done', kind: 'todo' }),
      }
    ));
    expect(external.status).toBe(200);
    expect(workItemRepository.findById(stashItem.id)?.status).toBe('done');
  });
});

const transcriptHomes: string[] = [];
afterEach(() => {
  while (transcriptHomes.length) rmSync(transcriptHomes.pop()!, { recursive: true, force: true });
});

for (const runtimeId of ['codex', 'claude-code'] as const) {
  test(`links a delayed ${runtimeId} launch transcript after a follow-up changed the stored prompt`, () => {
    const fixtureHome = realpathSync(mkdtempSync(join(tmpdir(), 'keepline-test-dispatch-transcript-')));
    transcriptHomes.push(fixtureHome);
    const bin = join(fixtureHome, 'bin');
    mkdirSync(bin);
    writeFileSync(join(bin, 'ps'), '#!/bin/sh\nprintf "PID %%CPU %%MEM TTY LSTART COMMAND\\n"\n', { mode: 0o755 });
    const child = Bun.spawnSync({
      cmd: [process.execPath, '--eval', `
        import { mkdirSync, writeFileSync } from 'fs';
        import { join } from 'path';
        import { resetDatabase } from './src/db/migrations.ts';
        import { workItemRepository } from './src/infrastructure/database/repositories/work-item.repository.ts';
        import { taskDispatchRepository } from './src/infrastructure/database/repositories/task-dispatch.repository.ts';
        import { sessionRepository } from './src/infrastructure/database/repositories/session.repository.ts';
        import { TaskDispatchService } from './src/services/task-dispatch.service.ts';
        import { serviceScanCommand } from './src/cli/service-scan.ts';
        resetDatabase();
        const runtimeId = ${JSON.stringify(runtimeId)};
        const fixtureHome = ${JSON.stringify(fixtureHome)};
        const launchedAt = new Date();
        let launchPrompt;
        const service = new TaskDispatchService({
          now: () => launchedAt,
          launch: (_executable, args) => { launchPrompt = args[0]; },
        });
        const item = workItemRepository.create({ title: 'Delayed transcript' });
        const dispatch = await service.dispatch(item.id, {
          runtimeId, cwd: fixtureHome, prompt: 'Launch task', idempotencyKey: 'delayed-transcript',
        });
        const firstId = '019ed4a3-2186-7e51-9aa1-ca1e376549b8';
        const laterId = '019ed4a3-2186-7e51-9aa1-ca1e376549b9';
        function writeTranscript(id, messages) {
          const directory = runtimeId === 'codex'
            ? join(fixtureHome, '.codex', 'sessions')
            : join(fixtureHome, '.claude', 'projects', '-fixture');
          mkdirSync(directory, { recursive: true });
          const entries = runtimeId === 'codex' ? [{
            type: 'session_meta', timestamp: launchedAt.toISOString(),
            payload: { id, cwd: fixtureHome },
          }] : [];
          for (const [index, content] of messages.entries()) {
            const timestamp = new Date(launchedAt.getTime() + index + 1).toISOString();
            entries.push(runtimeId === 'codex' ? {
              type: 'response_item', timestamp,
              payload: { type: 'message', role: 'user', content: [{ type: 'input_text', text: content }] },
            } : {
              type: 'user', uuid: 'user-' + index, sessionId: id, cwd: fixtureHome,
              timestamp, userType: 'external', message: { role: 'user', content },
            });
          }
          writeFileSync(join(directory, (runtimeId === 'codex' ? 'rollout-' : '') + id + '.jsonl'),
            entries.map(entry => JSON.stringify(entry)).join('\\n') + '\\n');
        }
        writeTranscript(firstId, ['An unrelated task']);
        await serviceScanCommand({ full: true });
        const first = taskDispatchRepository.findById(dispatch.id);
        const pendingBeforeMarker = taskDispatchRepository.findCorrelationPending().length;
        writeTranscript(laterId, [
          '<environment_context>fixture context</environment_context>',
          launchPrompt, 'Follow-up task already in transcript',
        ]);
        await serviceScanCommand({ full: true });
        const linked = taskDispatchRepository.findById(dispatch.id);
        const sessionId = runtimeId === 'codex' ? 'codex_' + laterId : laterId;
        const stored = sessionRepository.findBySessionId(sessionId);
        console.log(JSON.stringify({
          firstState: first.state, pendingBeforeMarker, linkedState: linked.state,
          linkedAgentSessionId: linked.linkedAgentSessionId, sessionId,
          storedPrompt: stored.initialPrompt, title: stored.title,
        }));
      `],
      cwd: process.cwd(),
      env: {
        ...process.env, HOME: fixtureHome, KEEPLINE_HOME: fixtureHome,
        KEEPLINE_TEST_HOME: fixtureHome, KEEPLINE_TEST_ISOLATED: '1',
        KEEPLINE_PROJECT_ROOTS: join(fixtureHome, '.claude', 'projects'),
        PATH: `${bin}:${process.env.PATH}`,
      },
      stdout: 'pipe', stderr: 'pipe',
    });
    if (child.exitCode !== 0) throw new Error(child.stderr.toString());
    const result = JSON.parse(child.stdout.toString().trim().split('\n').pop()!);
    expect(result.firstState).toBe('ambiguous');
    expect(result.linkedState).toBe('linked');
    expect(result.pendingBeforeMarker).toBe(1);
    expect(result.linkedAgentSessionId).toBe(encodeAgentSessionId(runtimeId, result.sessionId));
    expect(result.storedPrompt).toBe('Follow-up task already in transcript');
    expect(result.title).toBe('Follow-up task already in transcript');
  });
}
