/**
 * Web API Server for Tasker
 * Provides REST endpoints for session management
 *
 * All endpoints include input validation
 */

import { Hono } from 'hono';
import { cors } from 'hono/cors';
import { serveStatic } from 'hono/bun';
import path from 'path';
import { runMigrations } from '../../storage/migrations.js';
import { syncSessions, getAllSessions, completeSession } from '../../session/service.js';
import { getAggregatedSessions, getSessionStats } from '../../session/aggregator.js';
import { recoverSession, getRecoveryInfo } from '../../recovery/service.js';
import { stopProcess, isProcessRunning } from '../../process/scanner.js';
import { getSessionById, getAllSessions as getAllParsedSessions } from '../../claude/scanner.js';
import { initPricing } from '../../usage/pricing.js';
import { logger } from '../../utils/logger.js';
import {
  isValidSessionId,
  validateRecoverRequest,
  validateStopRequest,
} from './validation.js';

const app = new Hono();

// Simple in-memory rate limiter
const rateLimitStore = new Map<string, { count: number; resetTime: number }>();

function rateLimit(maxRequests: number, windowMs: number) {
  return async (c: any, next: () => Promise<void>) => {
    const ip = c.req.header('x-forwarded-for') || c.req.header('x-real-ip') || 'unknown';
    const now = Date.now();

    let record = rateLimitStore.get(ip);
    if (!record || now > record.resetTime) {
      record = { count: 0, resetTime: now + windowMs };
      rateLimitStore.set(ip, record);
    }

    record.count++;

    if (record.count > maxRequests) {
      logger.warn('Rate limit exceeded', { ip, count: record.count });
      return c.json({ success: false, error: 'Too many requests' }, 429);
    }

    await next();
  };
}

// Clean up rate limit store periodically (every 5 minutes)
setInterval(() => {
  const now = Date.now();
  for (const [key, value] of rateLimitStore.entries()) {
    if (now > value.resetTime) {
      rateLimitStore.delete(key);
    }
  }
}, 5 * 60 * 1000);

// Enable CORS
app.use('/*', cors());

// Rate limiting: 100 requests per minute for API routes
app.use('/api/*', rateLimit(100, 60 * 1000));

// Stricter rate limiting for write operations: 20 per minute
app.post('/api/sessions/*/recover', rateLimit(20, 60 * 1000));
app.post('/api/sessions/*/stop', rateLimit(20, 60 * 1000));
app.post('/api/sync', rateLimit(10, 60 * 1000));

// Serve static files (legacy)
app.use('/static/*', serveStatic({ root: './src/web/public' }));

// Serve React app assets - with path traversal protection
app.get('/assets/*', async (c) => {
  const requestPath = c.req.path;

  // Base directory for assets (absolute path)
  const basePath = path.resolve('./src/web/public/dist');

  // Normalize and resolve the requested path
  const normalizedPath = path.normalize(requestPath);
  const fullPath = path.resolve(basePath, '.' + normalizedPath);

  // Security: Ensure the resolved path is within the base directory
  if (!fullPath.startsWith(basePath + path.sep) && fullPath !== basePath) {
    logger.warn('Path traversal attempt blocked', { requestPath, fullPath });
    return c.notFound();
  }

  const file = Bun.file(fullPath);
  if (await file.exists()) {
    const contentType = requestPath.endsWith('.js') ? 'application/javascript' :
                        requestPath.endsWith('.css') ? 'text/css' :
                        'application/octet-stream';
    return new Response(file, {
      headers: { 'Content-Type': contentType },
    });
  }
  return c.notFound();
});

// API Routes
app.get('/api/sessions', async (c) => {
  try {
    // Parse pagination parameters
    const limit = Math.min(parseInt(c.req.query('limit') || '50'), 100);
    const offset = parseInt(c.req.query('offset') || '0');
    const status = c.req.query('status'); // Optional filter by status
    const sort = c.req.query('sort') || 'lastActiveAt';
    const order = c.req.query('order') === 'asc' ? 'asc' : 'desc';
    const fields = c.req.query('fields') || 'full'; // 'basic' or 'full'

    await syncSessions();
    let sessions = getAggregatedSessions();
    const stats = getSessionStats(sessions);

    // Filter by status if provided
    if (status) {
      sessions = sessions.filter(s => s.status === status);
    }

    // Sort sessions
    sessions.sort((a, b) => {
      let comparison = 0;
      if (sort === 'lastActiveAt') {
        comparison = a.lastActiveAt.getTime() - b.lastActiveAt.getTime();
      } else if (sort === 'directory') {
        comparison = a.directory.localeCompare(b.directory);
      } else if (sort === 'status') {
        comparison = a.status.localeCompare(b.status);
      }
      return order === 'desc' ? -comparison : comparison;
    });

    // Get total count before pagination
    const total = sessions.length;

    // Apply pagination
    const paginatedSessions = sessions.slice(offset, offset + limit);

    // For 'basic' mode, skip expensive usageStats calculation
    if (fields === 'basic') {
      return c.json({
        success: true,
        data: {
          sessions: paginatedSessions.map(s => ({
            id: s.id,
            sessionId: s.sessionId,
            directory: s.directory,
            status: s.status,
            title: s.title,
            lastActiveAt: s.lastActiveAt.toISOString(),
            startedAt: s.startedAt?.toISOString(),
            completedAt: s.completedAt?.toISOString(),
            createdAt: s.createdAt.toISOString(),
            updatedAt: s.updatedAt.toISOString(),
            pid: s.pid,
            tty: s.tty,
            toolCount: s.toolCount,
            messageCount: s.messageCount,
            processRunning: s.processRunning,
            cpuUsage: s.cpuUsage,
            memoryUsage: s.memoryUsage,
            // Omit heavy fields: initialPrompt, lastMessage, lastTool, lastToolInput, usageStats
          })),
          stats,
          pagination: {
            total,
            limit,
            offset,
            hasMore: offset + limit < total,
          },
        },
      });
    }

    // Full mode: include all fields with usage stats
    const parsedSessions = await getAllParsedSessions();
    const usageMap = new Map(
      parsedSessions.map(p => [p.sessionId, p.usageStats])
    );

    return c.json({
      success: true,
      data: {
        sessions: paginatedSessions.map(s => ({
          ...s,
          lastActiveAt: s.lastActiveAt.toISOString(),
          startedAt: s.startedAt?.toISOString(),
          completedAt: s.completedAt?.toISOString(),
          createdAt: s.createdAt.toISOString(),
          updatedAt: s.updatedAt.toISOString(),
          usageStats: usageMap.get(s.sessionId),
        })),
        stats,
        pagination: {
          total,
          limit,
          offset,
          hasMore: offset + limit < total,
        },
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

  // Validate session ID format
  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  // Parse and validate request body
  let body: unknown;
  try {
    body = await c.req.json();
  } catch {
    body = {};
  }

  const validation = validateRecoverRequest(body);
  if (!validation.valid) {
    return c.json({ success: false, error: validation.error }, 400);
  }

  const { method = 'resume', openTerminal = true, skipPermissions = false } = validation.data;

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
      method: method || 'resume',
      sessionId,
      directory: session.directory,
      openTerminal: openTerminal ?? true,
      skipPermissions: skipPermissions ?? false,
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

// Stop a session process (SIGTERM or SIGKILL)
app.post('/api/sessions/:id/stop', async (c) => {
  const sessionId = c.req.param('id');

  // Validate session ID format
  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  // Parse and validate request body
  let body: unknown;
  try {
    body = await c.req.json();
  } catch {
    body = {};
  }

  const validation = validateStopRequest(body);
  if (!validation.valid) {
    return c.json({ success: false, error: validation.error }, 400);
  }

  const { force = false } = validation.data;

  const sessions = getAllSessions();
  const session = sessions.find(s => s.sessionId === sessionId);

  if (!session) {
    return c.json({ success: false, error: 'Session not found' }, 404);
  }

  // For lost sessions, just mark as completed (no process to kill)
  if (session.status === 'lost' || !session.pid) {
    completeSession(sessionId);
    return c.json({
      success: true,
      message: 'Session cleared (no process was running)'
    });
  }

  // Check if process is still running
  if (!isProcessRunning(session.pid)) {
    completeSession(sessionId);
    return c.json({
      success: true,
      message: 'Process already terminated, session cleared'
    });
  }

  // Stop the process
  const result = stopProcess(session.pid, force);

  if (result.success) {
    // Mark session as completed after successful stop
    // Note: We do this optimistically - the process should terminate
    setTimeout(() => {
      if (!isProcessRunning(session.pid!)) {
        completeSession(sessionId);
      }
    }, 1000);

    return c.json({
      success: true,
      message: force ? 'Force kill signal sent' : 'Stop signal sent (will force kill in 5s if needed)'
    });
  }

  return c.json({ success: false, error: result.error }, 500);
});

// Get process status for a session
app.get('/api/sessions/:id/process', async (c) => {
  const sessionId = c.req.param('id');

  const sessions = getAllSessions();
  const session = sessions.find(s => s.sessionId === sessionId);

  if (!session) {
    return c.json({ success: false, error: 'Session not found' }, 404);
  }

  const processRunning = session.pid ? isProcessRunning(session.pid) : false;

  return c.json({
    success: true,
    data: {
      pid: session.pid,
      running: processRunning,
      status: session.status,
    }
  });
});

// Get tool calls for a session
app.get('/api/sessions/:id/tools', async (c) => {
  const sessionId = c.req.param('id');

  try {
    const parsedSession = await getSessionById(sessionId);

    if (!parsedSession) {
      return c.json({ success: false, error: 'Session not found' }, 404);
    }

    return c.json({
      success: true,
      data: {
        toolCalls: parsedSession.toolCalls || [],
        toolCount: parsedSession.toolCount,
      },
    });
  } catch (error) {
    logger.error('Failed to get tool calls', error);
    return c.json({ success: false, error: 'Failed to get tool calls' }, 500);
  }
});

// Get usage stats for a session
app.get('/api/sessions/:id/usage', async (c) => {
  const sessionId = c.req.param('id');

  try {
    const parsedSession = await getSessionById(sessionId);

    if (!parsedSession) {
      return c.json({ success: false, error: 'Session not found' }, 404);
    }

    return c.json({
      success: true,
      data: {
        usageStats: parsedSession.usageStats || {
          totalInputTokens: 0,
          totalOutputTokens: 0,
          totalTokens: 0,
          totalCost: 0,
          apiCalls: 0,
        },
      },
    });
  } catch (error) {
    logger.error('Failed to get usage stats', error);
    return c.json({ success: false, error: 'Failed to get usage stats' }, 500);
  }
});

// Get session details (lazy loaded fields)
app.get('/api/sessions/:id/details', async (c) => {
  const sessionId = c.req.param('id');

  // Validate session ID format
  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    // Get database session for basic fields
    const sessions = getAllSessions();
    const dbSession = sessions.find(s => s.sessionId === sessionId);

    if (!dbSession) {
      return c.json({ success: false, error: 'Session not found' }, 404);
    }

    // Get parsed session for detailed fields
    const parsedSession = await getSessionById(sessionId);

    return c.json({
      success: true,
      data: {
        initialPrompt: dbSession.initialPrompt || '',
        lastMessage: dbSession.lastMessage || '',
        lastTool: dbSession.lastTool || '',
        lastToolInput: dbSession.lastToolInput || '',
        currentFile: dbSession.currentFile || '',
        usageStats: parsedSession?.usageStats || {
          totalInputTokens: 0,
          totalOutputTokens: 0,
          totalTokens: 0,
          totalCost: 0,
          apiCalls: 0,
        },
      },
    });
  } catch (error) {
    logger.error('Failed to get session details', error);
    return c.json({ success: false, error: 'Failed to get session details' }, 500);
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

// Serve React app index.html for root
app.get('/', async () => {
  const file = Bun.file('./src/web/public/dist/index.html');
  return new Response(file, {
    headers: { 'Content-Type': 'text/html' },
  });
});

// Fallback to index.html for SPA routing
app.get('/*', async (c) => {
  const path = c.req.path;
  // Don't catch API or static asset routes
  if (path.startsWith('/api/') || path.startsWith('/assets/') || path.startsWith('/static/')) {
    return c.notFound();
  }
  const file = Bun.file('./src/web/public/dist/index.html');
  return new Response(file, {
    headers: { 'Content-Type': 'text/html' },
  });
});

export async function startWebServer(port: number = 3377) {
  runMigrations();

  // Initialize pricing from LiteLLM
  logger.info('Fetching model pricing from LiteLLM...');
  await initPricing();

  logger.info(`Starting web server on port ${port}`);

  Bun.serve({
    port,
    fetch: app.fetch,
  });

  console.log(`\n🌐 Tasker Web UI: http://localhost:${port}\n`);
}

export { app };
