import { Hono } from 'hono';
import { getHookAvailability } from '../../../adapters/hook/availability.js';
import { authMiddleware } from '../middleware/auth.js';
import { getRuntimeScanStatus } from '../../../services/runtime-status.js';
import { localServiceState } from '../../../local-api/service-state.js';
import { lastHookReceipt } from '../../../adapters/hook/observation.js';
import type { LedgerMonitorStatus } from '../../../domain/ledger/types.js';
import { isLifecycleHookInstalled, installHooks } from '../../../adapters/hook/installer.js';
import { logger } from '../../../lib/logger.js';

const app = new Hono();

app.use('*', authMiddleware);

app.get('/monitor', async (c) => {
  const receiver = localServiceState.lifecycleHook;
  const hooks = receiver.receiverRunning ? null : await getHookAvailability();
  const scan = localServiceState.scan;
  const data: LedgerMonitorStatus = {
    scan: {
      running: scan.running,
      lastStartedAt: scan.lastStartedAt?.toISOString(),
      lastCompletedAt: scan.lastCompletedAt?.toISOString(),
      intervalMs: scan.intervalMs,
      timeoutMs: scan.timeoutMs,
      failed: !!scan.lastError,
    },
    runtimes: getRuntimeScanStatus().map(runtime => ({
      runtimeId: runtime.runtimeId,
      lastScanAt: runtime.lastScanAt,
      errorCount: runtime.errorCount,
      hookInstalled: receiver.receiverRunning && receiver.port !== undefined
        ? isLifecycleHookInstalled(receiver.port, runtime.runtimeId)
        : !!hooks?.targets.some(target => target.runtimeId === runtime.runtimeId && target.installed),
      receiverRunning: receiver.receiverRunning || !!hooks?.receiverRunning,
      lastHookAt: lastHookReceipt(runtime.runtimeId === 'codex' ? 'codex' : 'claude'),
    })),
  };
  return c.json({ success: true, data });
});

app.get('/hooks', async (c) => {
  return c.json({
    success: true,
    data: await getHookAvailability(),
  });
});

// Only an explicit authenticated action installs hooks; monitoring never writes
// to runtime settings. Read back the result, including partial installations.
app.post('/hooks/install', async (c) => {
  try {
    installHooks();
    return c.json({ success: true, data: await getHookAvailability() });
  } catch (error) {
    logger.error('Hook installation failed', error);
    return c.json({ success: false, error: 'Hook 安装失败，请检查 Codex 和 Claude Code 配置文件的格式与写入权限，然后重试。' }, 500);
  }
});

export default app;
