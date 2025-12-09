/**
 * Web API Server for Tasker
 * Provides REST endpoints for session management
 */

import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { serveStatic } from 'hono/bun';
import { runMigrations } from '../../storage/migrations.js';
import { syncSessions, getAllSessions, completeSession } from '../../session/service.js';
import { getAggregatedSessions, getSessionStats } from '../../session/aggregator.js';
import { recoverSession, getRecoveryInfo } from '../../recovery/service.js';
import { logger } from '../../utils/logger.js';

const app = new Hono();

// Enable CORS
app.use('/*', cors());

// Serve static files
app.use('/static/*', serveStatic({ root: './src/web/public' }));

// API Routes
app.get('/api/sessions', async (c) => {
  try {
    await syncSessions();
    const sessions = getAggregatedSessions();
    const stats = getSessionStats(sessions);

    return c.json({
      success: true,
      data: {
        sessions: sessions.map(s => ({
          ...s,
          lastActiveAt: s.lastActiveAt.toISOString(),
          startedAt: s.startedAt?.toISOString(),
          completedAt: s.completedAt?.toISOString(),
          createdAt: s.createdAt.toISOString(),
          updatedAt: s.updatedAt.toISOString(),
        })),
        stats,
      },
    });
  } catch (error) {
    logger.error('Failed to get sessions', error);
    return c.json({ success: false, error: 'Failed to get sessions' }, 500);
  }
});

app.get('/api/sessions/:id', async (c) => {
  const sessionId = c.req.param('id');
  const sessions = getAllSessions();
  const session = sessions.find(s => s.sessionId === sessionId);

  if (!session) {
    return c.json({ success: false, error: 'Session not found' }, 404);
  }

  const recoveryInfo = getRecoveryInfo(sessionId);

  return c.json({
    success: true,
    data: {
      session: {
        ...session,
        lastActiveAt: session.lastActiveAt.toISOString(),
        startedAt: session.startedAt?.toISOString(),
        completedAt: session.completedAt?.toISOString(),
        createdAt: session.createdAt.toISOString(),
        updatedAt: session.updatedAt.toISOString(),
      },
      recovery: recoveryInfo,
    },
  });
});

app.post('/api/sessions/:id/recover', async (c) => {
  const sessionId = c.req.param('id');
  const body = await c.req.json();
  const { method = 'resume', openTerminal = true, skipPermissions = false } = body;

  const sessions = getAllSessions();
  const session = sessions.find(s => s.sessionId === sessionId);

  if (!session) {
    return c.json({ success: false, error: 'Session not found' }, 404);
  }

  const recoveryInfo = getRecoveryInfo(sessionId);

  if (!recoveryInfo.canRecover) {
    return c.json({ success: false, error: recoveryInfo.reason }, 400);
  }

  try {
    const result = await recoverSession({
      method: method as 'resume' | 'continue' | 'new',
      sessionId,
      directory: session.directory,
      openTerminal,
      skipPermissions,
    });

    return c.json({ success: result.success, error: result.error });
  } catch (error) {
    logger.error('Failed to recover session', error);
    return c.json({ success: false, error: 'Recovery failed' }, 500);
  }
});

app.post('/api/sessions/:id/complete', async (c) => {
  const sessionId = c.req.param('id');

  try {
    completeSession(sessionId);
    return c.json({ success: true });
  } catch (error) {
    logger.error('Failed to complete session', error);
    return c.json({ success: false, error: 'Failed to complete session' }, 500);
  }
});

app.post('/api/sync', async (c) => {
  try {
    const result = await syncSessions();
    return c.json({ success: true, data: result });
  } catch (error) {
    logger.error('Failed to sync sessions', error);
    return c.json({ success: false, error: 'Sync failed' }, 500);
  }
});

// Serve index.html for root
app.get('/', async () => {
  const file = Bun.file('./src/web/public/index.html');
  return new Response(file, {
    headers: { 'Content-Type': 'text/html' },
  });
});

export function startWebServer(port: number = 3377) {
  runMigrations();

  logger.info(`Starting web server on port ${port}`);

  Bun.serve({
    port,
    fetch: app.fetch,
  });

  console.log(`\n🌐 Tasker Web UI: http://localhost:${port}\n`);
}

export { app };
