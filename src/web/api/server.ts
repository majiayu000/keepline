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
import { runMigrations } from '../../db/migrations.js';
import { syncSessions } from '../../services/session.service.js';
import { getAggregatedSessions, getSessionStats } from '../../services/session.aggregator.js';
import { getAllSessions as getAllParsedSessions } from '../../adapters/claude/scanner.js';
import { initPricing } from '../../services/usage.pricing.js';
import { logger } from '../../lib/logger.js';
import { rateLimit } from './middleware/rateLimit.js';
import { sessions, recovery, usage } from './routes/index.js';
import { broadcast, wsClients, websocketHandler } from './websocket.js';

const app = new Hono();

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

// Mount route modules
app.route('/api/sessions', sessions);
app.route('/api/sessions', recovery);
app.route('/api', usage);

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

  logger.info(`Starting web server on port ${port}`);

  const server = Bun.serve({
    port,
    fetch(req, server) {
      const url = new URL(req.url);

      // Handle WebSocket upgrade
      if (url.pathname === '/ws') {
        const upgraded = server.upgrade(req, { data: undefined });
        if (upgraded) return undefined;
        return new Response('WebSocket upgrade failed', { status: 400 });
      }

      // Handle regular HTTP requests via Hono
      return app.fetch(req);
    },
    websocket: websocketHandler,
  });

  // Start periodic update checker (every 5 seconds)
  setInterval(checkAndBroadcastUpdates, 5000);

  logger.info(`Web UI available at http://localhost:${port}`);
  logger.info(`WebSocket available at ws://localhost:${port}/ws`);

  return server;
}

export { app };
