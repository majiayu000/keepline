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
import type { ServerWebSocket } from 'bun';
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

// WebSocket client management
interface WebSocketClient {
  ws: ServerWebSocket<unknown>;
  subscribedTo: Set<string>; // session IDs or 'all'
}

const wsClients = new Set<WebSocketClient>();

// Broadcast message to all connected clients
function broadcast(type: string, data: unknown) {
  const message = JSON.stringify({ type, data, timestamp: new Date().toISOString() });
  for (const client of wsClients) {
    try {
      client.ws.send(message);
    } catch (error) {
      logger.error('Failed to send WebSocket message', error);
    }
  }
}

// Broadcast to clients subscribed to a specific session
function broadcastToSession(sessionId: string, type: string, data: unknown) {
  const message = JSON.stringify({ type, data, sessionId, timestamp: new Date().toISOString() });
  for (const client of wsClients) {
    if (client.subscribedTo.has('all') || client.subscribedTo.has(sessionId)) {
      try {
        client.ws.send(message);
      } catch (error) {
        logger.error('Failed to send WebSocket message', error);
      }
    }
  }
}

// Export for use by other modules (e.g., hooks)
export { broadcast, broadcastToSession };

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

// Rate limiting: 500 requests per minute for API routes (local tool, be generous)
app.use('/api/*', rateLimit(500, 60 * 1000));

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

// Track last sync time for smart sync
let lastSyncTime = 0;
const SYNC_INTERVAL_MS = 30000; // Only auto-sync every 30 seconds
let isSyncingInBackground = false;

// Background sync function (non-blocking)
async function backgroundSync() {
  if (isSyncingInBackground) return;
  isSyncingInBackground = true;
  try {
    await syncSessions();
    lastSyncTime = Date.now();
    // Broadcast update to WebSocket clients after sync
    broadcastToClients({ type: 'sync:complete' });
  } catch (error) {
    // Better error logging
    const errorMessage = error instanceof Error ? error.message : String(error);
    const errorStack = error instanceof Error ? error.stack : undefined;
    logger.error('Background sync failed', { message: errorMessage, stack: errorStack });
  } finally {
    isSyncingInBackground = false;
  }
}

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
    const skipSync = c.req.query('skipSync') === 'true'; // Skip sync for pagination requests

    // Smart sync: trigger background sync if needed (non-blocking)
    const now = Date.now();
    if (!skipSync && (now - lastSyncTime > SYNC_INTERVAL_MS)) {
      // Don't await - let it run in background
      backgroundSync();
    }

    // Return data immediately from database (fast)
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

// Get sub-agents for a session
app.get('/api/sessions/:id/subagents', async (c) => {
  const sessionId = c.req.param('id');

  // Validate session ID format
  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    // Get all sessions including sub-agents
    const allSessions = await getAllParsedSessions({ includeSubAgents: true });

    // Find sub-agents that belong to this parent session
    const subAgents = allSessions
      .filter(s => s.parentSessionId === sessionId)
      .map(s => ({
        sessionId: s.sessionId,
        agentId: s.agentId,
        directory: s.directory,
        firstMessage: s.firstMessage,
        lastMessage: s.lastMessage,
        messageCount: s.messageCount,
        toolCount: s.toolCount,
        lastTool: s.lastTool,
        startedAt: s.startedAt?.toISOString(),
        lastActiveAt: s.lastActiveAt.toISOString(),
        usageStats: s.usageStats,
      }));

    return c.json({
      success: true,
      data: {
        parentSessionId: sessionId,
        subAgents,
        count: subAgents.length,
      },
    });
  } catch (error) {
    logger.error('Failed to get sub-agents', error);
    return c.json({ success: false, error: 'Failed to get sub-agents' }, 500);
  }
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

// Get full session data (combines details, tools, subagents in one request)
// This reduces 3 API calls to 1 when expanding a session card
app.get('/api/sessions/:id/full', async (c) => {
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

    // Get parsed session for detailed fields and tool calls
    const parsedSession = await getSessionById(sessionId);

    // Get sub-agents
    const allSessions = await getAllParsedSessions({ includeSubAgents: true });
    const subAgents = allSessions
      .filter(s => s.parentSessionId === sessionId)
      .map(s => ({
        sessionId: s.sessionId,
        agentId: s.agentId,
        directory: s.directory,
        firstMessage: s.firstMessage,
        lastMessage: s.lastMessage,
        messageCount: s.messageCount,
        toolCount: s.toolCount,
        lastTool: s.lastTool,
        startedAt: s.startedAt?.toISOString(),
        lastActiveAt: s.lastActiveAt.toISOString(),
        usageStats: s.usageStats,
      }));

    return c.json({
      success: true,
      data: {
        // Details
        details: {
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
        // Tool calls
        tools: {
          toolCalls: parsedSession?.toolCalls || [],
          toolCount: parsedSession?.toolCount || 0,
        },
        // Sub-agents
        subAgents: {
          subAgents,
          count: subAgents.length,
        },
      },
    });
  } catch (error) {
    logger.error('Failed to get full session data', error);
    return c.json({ success: false, error: 'Failed to get full session data' }, 500);
  }
});

app.post('/api/sync', async (c) => {
  try {
    // Parse request body for sync options
    let body: { fullSync?: boolean } = {};
    try {
      body = await c.req.json();
    } catch {
      // Empty body is fine
    }

    // Full sync scans all files, regular sync only scans last 7 days
    const result = await syncSessions({ fullSync: body.fullSync });
    lastSyncTime = Date.now(); // Reset sync timer after manual sync

    return c.json({ success: true, data: result });
  } catch (error) {
    logger.error('Failed to sync sessions', error);
    return c.json({ success: false, error: 'Sync failed' }, 500);
  }
});

// Get Claude Code quota/rate limits from OAuth API
app.get('/api/quota', async (c) => {
  try {
    // Try to get OAuth token from macOS Keychain
    const proc = Bun.spawn(['security', 'find-generic-password', '-s', 'Claude Code-credentials', '-w'], {
      stdout: 'pipe',
      stderr: 'pipe',
    });

    const output = await new Response(proc.stdout).text();
    const exitCode = await proc.exited;

    if (exitCode !== 0 || !output.trim()) {
      return c.json({
        success: false,
        error: 'OAuth token not found. Please ensure you are logged into Claude Code.'
      }, 401);
    }

    // Parse the credentials JSON
    let credentials: { claudeAiOauth?: { accessToken?: string } };
    try {
      credentials = JSON.parse(output.trim());
    } catch {
      return c.json({ success: false, error: 'Failed to parse credentials' }, 500);
    }

    const accessToken = credentials.claudeAiOauth?.accessToken;
    if (!accessToken) {
      return c.json({ success: false, error: 'OAuth access token not found' }, 401);
    }

    // Use curl to fetch from Anthropic OAuth API (works better with their security)
    const curlProc = Bun.spawn([
      'curl', '-s',
      '-H', 'Accept: application/json',
      '-H', `Authorization: Bearer ${accessToken}`,
      '-H', 'anthropic-beta: oauth-2025-04-20',
      'https://api.anthropic.com/api/oauth/usage'
    ], {
      stdout: 'pipe',
      stderr: 'pipe',
    });

    const curlOutput = await new Response(curlProc.stdout).text();
    const curlExitCode = await curlProc.exited;

    if (curlExitCode !== 0) {
      const curlStderr = await new Response(curlProc.stderr).text();
      logger.error('Quota curl failed', { exitCode: curlExitCode, stderr: curlStderr });
      return c.json({ success: false, error: 'Failed to fetch quota' }, 500);
    }

    // Parse the response
    let data;
    try {
      data = JSON.parse(curlOutput);
    } catch {
      logger.error('Failed to parse quota response', { output: curlOutput });
      return c.json({ success: false, error: 'Invalid quota response' }, 500);
    }

    // Check for error in response
    if (data.error) {
      logger.error('Quota API error', { error: data.error });
      return c.json({
        success: false,
        error: data.error.message || 'Quota API error'
      }, 403);
    }

    return c.json({ success: true, data });
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.error('Failed to get quota', { message: errorMessage });
    return c.json({ success: false, error: 'Failed to get quota data' }, 500);
  }
});

// Get usage analytics from ccusage CLI tool
app.get('/api/usage', async (c) => {
  try {
    const type = c.req.query('type') || 'daily'; // daily, monthly, weekly, session
    const since = c.req.query('since'); // YYYYMMDD format
    const until = c.req.query('until'); // YYYYMMDD format

    // Build ccusage command
    const args = [type, '--json'];
    if (since) args.push('--since', since);
    if (until) args.push('--until', until);

    // Execute ccusage
    const proc = Bun.spawn(['npx', 'ccusage', ...args], {
      stdout: 'pipe',
      stderr: 'pipe',
    });

    const output = await new Response(proc.stdout).text();
    const exitCode = await proc.exited;

    if (exitCode !== 0) {
      const stderr = await new Response(proc.stderr).text();
      logger.error('ccusage failed', { exitCode, stderr });
      return c.json({ success: false, error: 'Failed to get usage data' }, 500);
    }

    const data = JSON.parse(output);
    return c.json({ success: true, data });
  } catch (error) {
    logger.error('Failed to get usage data', error);
    return c.json({ success: false, error: 'Failed to get usage data' }, 500);
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

// Session state tracking for real-time updates
let previousSessionsState: string = '';

async function checkAndBroadcastUpdates() {
  try {
    if (wsClients.size === 0) return; // No clients, skip

    await syncSessions();
    const sessions = getAggregatedSessions();
    const stats = getSessionStats(sessions);

    // Create a simple hash of current state
    const currentState = JSON.stringify({
      stats,
      sessionIds: sessions.map(s => `${s.sessionId}:${s.status}`).sort(),
    });

    // Only broadcast if state changed
    if (currentState !== previousSessionsState) {
      previousSessionsState = currentState;

      // Get usage stats for full data
      const parsedSessions = await getAllParsedSessions();
      const usageMap = new Map(
        parsedSessions.map(p => [p.sessionId, p.usageStats])
      );

      broadcast('sessions:update', {
        sessions: sessions.map(s => ({
          ...s,
          lastActiveAt: s.lastActiveAt.toISOString(),
          startedAt: s.startedAt?.toISOString(),
          completedAt: s.completedAt?.toISOString(),
          createdAt: s.createdAt.toISOString(),
          updatedAt: s.updatedAt.toISOString(),
          usageStats: usageMap.get(s.sessionId),
        })),
        stats,
      });
    }
  } catch (error) {
    logger.error('Failed to check for updates', error);
  }
}

export async function startWebServer(port: number = 3377) {
  runMigrations();

  // Initialize pricing from LiteLLM
  logger.info('Fetching model pricing from LiteLLM...');
  await initPricing();

  // Initial sync on startup (so database has data for first request)
  logger.info('Running initial session sync...');
  await syncSessions();
  lastSyncTime = Date.now();

  logger.info(`Starting web server on port ${port}`);

  const server = Bun.serve({
    port,
    fetch(req, server) {
      const url = new URL(req.url);

      // Handle WebSocket upgrade
      if (url.pathname === '/ws') {
        const upgraded = server.upgrade(req);
        if (upgraded) return undefined;
        return new Response('WebSocket upgrade failed', { status: 400 });
      }

      // Handle regular HTTP requests via Hono
      return app.fetch(req, server);
    },
    websocket: {
      open(ws) {
        const client: WebSocketClient = {
          ws,
          subscribedTo: new Set(['all']),
        };
        wsClients.add(client);
        (ws as any).__client = client;
        logger.info(`WebSocket client connected (${wsClients.size} total)`);

        // Send initial data
        ws.send(JSON.stringify({
          type: 'connected',
          data: { clientCount: wsClients.size },
          timestamp: new Date().toISOString(),
        }));
      },
      message(ws, message) {
        try {
          const data = JSON.parse(message.toString());
          const client = (ws as any).__client as WebSocketClient;

          // Handle subscription messages
          if (data.type === 'subscribe') {
            if (data.sessionId) {
              client.subscribedTo.add(data.sessionId);
            }
          } else if (data.type === 'unsubscribe') {
            if (data.sessionId) {
              client.subscribedTo.delete(data.sessionId);
            }
          } else if (data.type === 'ping') {
            ws.send(JSON.stringify({ type: 'pong', timestamp: new Date().toISOString() }));
          }
        } catch (error) {
          logger.error('Failed to parse WebSocket message', error);
        }
      },
      close(ws) {
        const client = (ws as any).__client as WebSocketClient;
        if (client) {
          wsClients.delete(client);
        }
        logger.info(`WebSocket client disconnected (${wsClients.size} remaining)`);
      },
    },
  });

  // Start periodic update checker (every 5 seconds)
  setInterval(checkAndBroadcastUpdates, 5000);

  console.log(`\n🌐 Tasker Web UI: http://localhost:${port}`);
  console.log(`📡 WebSocket: ws://localhost:${port}/ws\n`);

  return server;
}

export { app };
