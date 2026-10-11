import { existsSync, watch, type FSWatcher } from 'fs';
import { dirname, join, relative, sep } from 'path';
import { CLAUDE_PROJECT_ROOTS, CODEX_SESSIONS } from '../lib/paths.js';
import { mountServiceClient } from '../web/api/service-client.js';
import { replayHookSpool } from '../adapters/hook/spool.js';
import { broadcast, websocketHandler } from '../web/api/websocket.js';
import { verifyToken } from './auth.service.js';
import { setWebSessionSource } from '../web/api/session-source.js';
import type { Server } from 'bun';
import {
  startLifecycleReceiver,
  type LifecycleReceiver,
} from '../adapters/hook/completion-receiver.js';
import { runServiceMigrations } from '../local-api/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { logger } from '../lib/logger.js';
import { config } from '../lib/config.js';
import { events } from '../lib/events.js';
import { createLocalApiApp } from '../local-api/app.js';
import { createRecoveryProcessRunner } from '../local-api/routes/recovery.js';
import { localServiceState } from '../local-api/service-state.js';
import { replaceRuntimeScanStatus, type RuntimeScanSummary } from './runtime-status.js';
import {
  beginSessionReconciliation,
  completeSessionReconciliation,
  failSessionReconciliation,
  invalidateSessionClaims,
} from './session-reconciliation-gate.js';

const SCAN_RESULT_PREFIX = '__KEEPLINE_SERVICE_SCAN__';

export interface KeeplineService {
  server: Server<unknown>;
  hookPort: number;
  stop(): Promise<void>;
}

export interface KeeplineServiceOptions {
  port?: number;
  hookPort?: number;
  /** Periodic transcript scan interval. Zero disables the periodic timer. */
  scanIntervalMs?: number;
  scanTimeoutMs?: number;
  /**
   * Timeout for the unbounded startup `--full` reconciliation scan.
   * Defaults higher than the periodic scan timeout so large histories can finish.
   */
  initialScanTimeoutMs?: number;
  scanKillGraceMs?: number;
  scanOutputLimitBytes?: number;
  /** Test/support override. Production resolves the isolated scan from the current entrypoint. */
  scanCommand?: string[];
  /** Test/support override for the first complete reconciliation scan. */
  initialScanCommand?: string[];
  /** Test/support override. Production resolves recovery through an isolated child process. */
  recoveryCommand?: string[];
}

const DEFAULT_SCAN_TIMEOUT_MS = 30_000;
/** Allow complete startup reconciliation to exceed the bounded periodic timeout. */
const DEFAULT_INITIAL_SCAN_TIMEOUT_MS = 5 * 60_000;
const DEFAULT_SCAN_KILL_GRACE_MS = 1_000;
const DEFAULT_SCAN_OUTPUT_LIMIT_BYTES = 512 * 1024;
/** Retry delay when the first reconciliation fails and periodic scanning is disabled. */
const STARTUP_SCAN_RETRY_MS = 3_000;

function isAllowedLoopbackRequestHost(req: Request, port: number): boolean {
  const hostHeader = req.headers.get('host');
  if (!hostHeader) return false;
  try {
    const url = new URL(`http://${hostHeader}`);
    const hostname = url.hostname.toLowerCase();
    const isLoopback = hostname === '127.0.0.1' || hostname === 'localhost' ||
      hostname === '[::1]' || hostname === '::1';
    return isLoopback && (!url.port || Number(url.port) === port);
  } catch {
    return false;
  }
}

async function readBoundedText(
  stream: ReadableStream<Uint8Array>,
  byteLimit: number
): Promise<string> {
  const reader = stream.getReader();
  let retained = new Uint8Array(0);
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value.byteLength >= byteLimit) {
        retained = value.slice(value.byteLength - byteLimit);
        continue;
      }
      const keepFromPrevious = Math.min(retained.byteLength, byteLimit - value.byteLength);
      const next = new Uint8Array(keepFromPrevious + value.byteLength);
      next.set(retained.slice(retained.byteLength - keepFromPrevious));
      next.set(value, keepFromPrevious);
      retained = next;
    }
  } finally {
    reader.releaseLock();
  }
  return new TextDecoder().decode(retained);
}

function waitForExit(
  process: ReturnType<typeof Bun.spawn>,
  timeoutMs: number
): Promise<boolean> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (value: boolean) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(value);
    };
    const timer = setTimeout(() => finish(false), timeoutMs);
    process.exited.then(() => finish(true), () => finish(true));
  });
}

async function terminateProcess(
  process: ReturnType<typeof Bun.spawn>,
  graceMs: number
): Promise<void> {
  if (await waitForExit(process, 0)) return;
  process.kill('SIGTERM');
  if (await waitForExit(process, graceMs)) return;
  process.kill('SIGKILL');
  await waitForExit(process, graceMs);
}

export async function startKeeplineService(
  options: KeeplineServiceOptions | number = {}
): Promise<KeeplineService> {
  const port = typeof options === 'number' ? options : (options.port ?? 3377);
  const hookPort = typeof options === 'number'
    ? config.get().hookPort
    : (options.hookPort ?? config.get().hookPort);
  const configuredScanInterval = typeof options === 'number'
    ? 5_000
    : (options.scanIntervalMs ?? 5_000);
  const scanTimeoutMs = typeof options === 'number'
    ? DEFAULT_SCAN_TIMEOUT_MS
    : (options.scanTimeoutMs ?? DEFAULT_SCAN_TIMEOUT_MS);
  const initialScanTimeoutMs = typeof options === 'number'
    ? DEFAULT_INITIAL_SCAN_TIMEOUT_MS
    : (options.initialScanTimeoutMs ?? DEFAULT_INITIAL_SCAN_TIMEOUT_MS);
  const scanKillGraceMs = typeof options === 'number'
    ? DEFAULT_SCAN_KILL_GRACE_MS
    : (options.scanKillGraceMs ?? DEFAULT_SCAN_KILL_GRACE_MS);
  const scanOutputLimitBytes = typeof options === 'number'
    ? DEFAULT_SCAN_OUTPUT_LIMIT_BYTES
    : (options.scanOutputLimitBytes ?? DEFAULT_SCAN_OUTPUT_LIMIT_BYTES);
  if (!Number.isFinite(configuredScanInterval) || configuredScanInterval < 0) {
    throw new Error('Service scan interval must be zero or a positive number');
  }
  if (!Number.isInteger(hookPort) || hookPort < 0 || hookPort > 65535) {
    throw new Error('Invalid completion hook port');
  }
  if (!Number.isFinite(scanTimeoutMs) || scanTimeoutMs <= 0 ||
      !Number.isFinite(initialScanTimeoutMs) || initialScanTimeoutMs <= 0 ||
      !Number.isFinite(scanKillGraceMs) || scanKillGraceMs < 0 ||
      !Number.isInteger(scanOutputLimitBytes) || scanOutputLimitBytes < 1_024) {
    throw new Error('Invalid service scan process limits');
  }
  const entrypoint = process.argv[1];
  const configuredRecoveryCommand = typeof options === 'number'
    ? undefined
    : options.recoveryCommand;
  const recoveryCommand = configuredRecoveryCommand ??
    (entrypoint ? [process.execPath, entrypoint, '_service-recovery'] : []);
  runServiceMigrations();
  localServiceState.scan.running = false;
  localServiceState.scan.completed = false;
  localServiceState.scan.lastStartedAt = undefined;
  localServiceState.scan.lastCompletedAt = undefined;
  localServiceState.scan.lastError = undefined;
  setWebSessionSource('service');
  const app = createLocalApiApp({
    recoveryRunner: createRecoveryProcessRunner(recoveryCommand),
  });
  const disposeClient = mountServiceClient(app);
  const hostname = '127.0.0.1';
  const server = Bun.serve({
    hostname,
    port,
    fetch(req, bunServer) {
      if (!isAllowedLoopbackRequestHost(req, bunServer.port ?? port)) {
        return new Response('Forbidden', { status: 403 });
      }
      const pathname = new URL(req.url).pathname;
      if (pathname === '/ws') {
        const token = new URL(req.url).searchParams.get('token');
        if (!token || !verifyToken(token)) return new Response('Unauthorized',{ status: 401 });
        return bunServer.upgrade(req,{ data: { token,sessionIds: new Set<string>() } }) ? undefined : new Response('Upgrade failed',{ status: 400 });
      }
      if (!localServiceState.scan.completed && pathname.startsWith('/api/') && !pathname.startsWith('/api/auth/') &&
          pathname !== '/api/v1/health' &&
          pathname !== '/api/v1/meta' &&
          pathname !== '/api/v1/auth/local') {
        return Response.json(
          {
            success: false,
            error: localServiceState.scan.lastError
              ? `Startup reconciliation failed: ${localServiceState.scan.lastError}`
              : 'Startup reconciliation is still running',
          },
          { status: 503 }
        );
      }
      return app.fetch(req, { server: bunServer });
    },
    websocket: websocketHandler,
  });
  let lifecycleReceiver: LifecycleReceiver;
  try {
    lifecycleReceiver = startLifecycleReceiver(hookPort);
  } catch (error) {
    server.stop(true);
    closeDatabase();
    throw error;
  }
  localServiceState.lifecycleHook.receiverRunning = true;
  localServiceState.lifecycleHook.port = lifecycleReceiver.port;
  const replaySpool = () => replayHookSpool(async event => {
    const response = await fetch(`http://127.0.0.1:${lifecycleReceiver.port}/hook?runtime=${event.runtime}`,{ method: 'POST',headers: { 'Content-Type': 'application/json','X-Keepline-Event-Id': event.id },body: JSON.stringify({ ...event.payload,timestamp: event.payload.timestamp ?? event.receivedAt }) });
    if (response.ok) return true;
    if (response.status === 400 || response.status === 413 || response.status === 422) {
      logger.warn('Hook spool delivery rejected',{ id: event.id,status: response.status }); return 'reject';
    }
    return false;
  });
  let reconciliationToken: string | undefined;
  try {
    reconciliationToken = beginSessionReconciliation('service');
    const interruptedSessions = invalidateSessionClaims(reconciliationToken);
    if (interruptedSessions > 0) {
      logger.info(
        `Marked ${interruptedSessions} persisted live session(s) interrupted before reconciliation`
      );
    }
  } catch (error) {
    if (reconciliationToken) {
      failSessionReconciliation(
        reconciliationToken,
        error instanceof Error ? error.message : String(error)
      );
    }
    lifecycleReceiver.stop();
    localServiceState.lifecycleHook.receiverRunning = false;
    localServiceState.lifecycleHook.port = undefined;
    server.stop(true);
    closeDatabase();
    throw error;
  }

  let stopped = false;
  let scanTimer: ReturnType<typeof setTimeout> | undefined;
  let nextScanAt = 0;
  let scanPromise: Promise<void> | undefined;
  let scanProcess: ReturnType<typeof Bun.spawn> | undefined;
  let nextScanDelayMs = configuredScanInterval;
  let continueCorrelation = false;
  let rescanRequested = false;
  let rescanUrgent = false;
  const scan = async () => {
    if (stopped) return;
    if (localServiceState.scan.running) {
      rescanRequested = true;
      return;
    }
    const isInitialScan = !localServiceState.scan.completed;
    const activeScanTimeoutMs = isInitialScan ? initialScanTimeoutMs : scanTimeoutMs;
    const startedAt = Date.now();
    localServiceState.scan.running = true;
    localServiceState.scan.lastStartedAt = new Date();
    localServiceState.scan.lastError = undefined;
    logger.info('Service scan started', { full: isInitialScan, timeoutMs: activeScanTimeoutMs });
    try {
      await replaySpool();
      const command = typeof options === 'number'
        ? undefined
        : localServiceState.scan.completed
          ? options.scanCommand
          : (options.initialScanCommand ?? options.scanCommand);
      if (!command && !entrypoint) throw new Error('Unable to resolve Keepline service entrypoint');
      const child = Bun.spawn(
        command ?? [
          process.execPath,
          entrypoint!,
          '_service-scan',
          ...(localServiceState.scan.completed ? [] : ['--full']),
        ],
        {
          env: process.env,
          stdout: 'pipe',
          stderr: 'pipe',
        }
      );
      scanProcess = child;
      if (!child.stdout || typeof child.stdout === 'number' ||
          !child.stderr || typeof child.stderr === 'number') {
        child.kill('SIGTERM');
        throw new Error('Unable to capture isolated session scan output');
      }
      const stdoutPromise = readBoundedText(child.stdout, scanOutputLimitBytes);
      const stderrPromise = readBoundedText(child.stderr, scanOutputLimitBytes);
      if (!await waitForExit(child, activeScanTimeoutMs)) {
        await terminateProcess(child, scanKillGraceMs);
        const stderr = await stderrPromise;
        await stdoutPromise;
        throw new Error(
          `Session scan timed out after ${activeScanTimeoutMs} ms${stderr.trim() ? `: ${stderr.trim()}` : ''}`
        );
      }
      const exitCode = await child.exited;
      const [stdout, stderr] = await Promise.all([stdoutPromise, stderrPromise]);
      if (scanProcess === child) scanProcess = undefined;
      if (exitCode !== 0) {
        throw new Error(stderr.trim() || `Session scan exited with status ${exitCode}`);
      }
      const resultLine = stdout.split('\n').find((line) => line.startsWith(SCAN_RESULT_PREFIX));
      if (!resultLine) throw new Error('Session scan returned no result payload');
      const payload = JSON.parse(resultLine.slice(SCAN_RESULT_PREFIX.length)) as {
        runtimeScan?: RuntimeScanSummary[];
        pendingDispatches?: number;
        summaryCache?: { hits: number; misses: number; writes: number };
        cpuMicros?: { user: number; system: number };
        incremental?: { resumed: number; bytes: number };
      };
      if (Array.isArray(payload.runtimeScan)) {
        replaceRuntimeScanStatus(payload.runtimeScan);
      }
      continueCorrelation = typeof payload.pendingDispatches === 'number' &&
        payload.pendingDispatches > 0;
      nextScanDelayMs = continueCorrelation
        ? (configuredScanInterval === 0 ? 3_000 : Math.min(configuredScanInterval, 3_000))
        : configuredScanInterval;
      await replaySpool();
      localServiceState.scan.completed = true;
      localServiceState.scan.lastCompletedAt = new Date();
      completeSessionReconciliation(reconciliationToken);
      broadcast('ledger:update',{});
      broadcast('sync:complete',{ timestamp: new Date().toISOString() });
      logger.info('Service scan completed', {
        full: isInitialScan, elapsedMs: Date.now() - startedAt, summaryCache: payload.summaryCache, cpuMicros: payload.cpuMicros, incremental: payload.incremental,
      });
    } catch (error) {
      localServiceState.scan.lastError = error instanceof Error ? error.message : String(error);
      if (isInitialScan) {
        failSessionReconciliation(reconciliationToken, localServiceState.scan.lastError);
      }
      if (!stopped) logger.error('Service scan failed', {
        full: isInitialScan, elapsedMs: Date.now() - startedAt, message: localServiceState.scan.lastError,
      });
    } finally {
      scanProcess = undefined;
      localServiceState.scan.running = false;
    }
  };

  const scheduleScan = (delayMs: number) => {
    if (stopped) return;
    if (scanTimer) clearTimeout(scanTimer);
    nextScanAt = Date.now() + delayMs;
    scanTimer = setTimeout(() => {
      scanTimer = undefined;
      scanPromise = scan().finally(() => {
        scanPromise = undefined;
        if (stopped) return;
        if (rescanRequested) {
          rescanRequested = false;
          const delay = rescanUrgent ? 250 : Math.max(250,2_000 - (Date.now() - (localServiceState.scan.lastStartedAt?.getTime() ?? 0)));
          rescanUrgent = false;
          scheduleScan(delay);
        } else if (!localServiceState.scan.completed) {
          // Keep retrying startup reconciliation even when --scan-interval 0.
          scheduleScan(STARTUP_SCAN_RETRY_MS);
        } else if (continueCorrelation || configuredScanInterval > 0) {
          scheduleScan(nextScanDelayMs);
        }
      });
    }, delayMs);
  };

  // Leave a short window for health/meta requests before transcript parsing begins.
  scheduleScan(250);
  const requestScan = () => {
    if (stopped) return;
    if (localServiceState.scan.running) {
      rescanRequested = true;
      rescanUrgent = true;
      return;
    }
    scheduleScan(250);
  };
  const requestFileScan = () => {
    if (stopped) return;
    if (localServiceState.scan.running) { rescanRequested = true; return; }
    const delay = Math.max(250,2_000 - (Date.now() - (localServiceState.scan.lastStartedAt?.getTime() ?? 0)));
    if (!scanTimer || Date.now() + delay < nextScanAt) scheduleScan(delay);
  };
  // Watch transcript data only; caches and the dashboard's own files cannot trigger scans.
  const transcriptWatchers: FSWatcher[] = [];
  if (configuredScanInterval > 0) for (const root of new Set([...CLAUDE_PROJECT_ROOTS,CODEX_SESSIONS])) {
    let directory = root;
    while (!existsSync(directory) && dirname(directory) !== directory) directory = dirname(directory);
    try {
      const watcher = watch(directory,{ recursive: true },(_event,filename) => {
        if (!filename) { requestFileScan(); return; }
        const changed = join(directory,filename.toString());
        const rel = relative(root,changed);
        if (rel !== '..' && !rel.startsWith(`..${sep}`) && (changed.endsWith('.jsonl') || _event === 'rename')) requestFileScan();
      });
      watcher.on('error',error => logger.warn('Transcript watch failed; periodic reconciliation remains active',{ root,message: error.message }));
      transcriptWatchers.push(watcher);
    } catch (error) {
      logger.warn('Unable to watch transcripts; periodic reconciliation remains active',{ root,message: error instanceof Error ? error.message : String(error) });
    }
  }
  events.on('session:updated', requestScan);
  events.on('session:discovered', requestScan);
  events.on('dispatch:created', requestScan);
  events.on('session:turn-ended', requestScan);
  events.on('session:completed', requestScan);

  logger.info(
    `Keepline service available at http://${hostname}:${server.port} ` +
    `(lifecycle hooks: ${lifecycleReceiver.port})`
  );
  return {
    server,
    hookPort: lifecycleReceiver.port,
    async stop() {
      if (stopped) return;
      stopped = true;
      disposeClient();
      if (scanTimer) clearTimeout(scanTimer);
      for (const watcher of transcriptWatchers) watcher.close();
      events.off('session:updated', requestScan);
      events.off('session:discovered', requestScan);
      events.off('dispatch:created', requestScan);
      events.off('session:turn-ended', requestScan);
      events.off('session:completed', requestScan);
      if (scanProcess) await terminateProcess(scanProcess, scanKillGraceMs);
      if (scanPromise) {
        await Promise.race([
          scanPromise,
          new Promise<void>((resolve) => setTimeout(resolve, scanKillGraceMs * 2 + 100)),
        ]);
      }
      if (!localServiceState.scan.completed) {
        failSessionReconciliation(reconciliationToken, 'Service stopped before reconciliation completed');
      }
      try {
        lifecycleReceiver.stop();
      } finally {
        localServiceState.lifecycleHook.receiverRunning = false;
        localServiceState.lifecycleHook.port = undefined;
        server.stop(true);
        setWebSessionSource('standalone');
        closeDatabase();
      }
    },
  };
}
