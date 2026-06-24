/**
 * Web API Server for Keepline
 * Provides REST endpoints for session management
 *
 * All endpoints include input validation
 */

import { Hono } from 'hono';
import { serveStatic } from 'hono/bun';
import { existsSync } from 'fs';
import path from 'path';
import { runMigrations } from '../../db/migrations.js';
import { syncSessions } from '../../services/session.service.js';
import {
  getAggregatedSessionsBasic,
  getSessionStats,
} from '../../services/session.aggregator.js';
import { initPricing } from '../../services/usage.pricing.js';
import { initializeMemoryService } from '../../services/memory.service.js';
import { logger } from '../../lib/logger.js';
import { rateLimit } from './middleware/rateLimit.js';
import {
  sessions,
  recovery,
  usage,
  memory,
  plans,
  auth,
  projects,
  workItems,
  workItemEvidence,
} from './routes/index.js';
import { broadcast, wsClients, websocketHandler } from './websocket.js';
import { terminalWebsocketHandler } from './terminal-websocket.js';
import { verifyToken } from '../../services/auth.service.js';
import { config } from '../../lib/config.js';
import { serializeBasicSessions } from './session-response.js';
import {
  REALTIME_FULL_SYNC_INTERVAL_MS,
  REALTIME_POLL_INTERVAL_MS,
  shouldRunRealtimeFullSync,
} from './realtime-updates.js';
import { isAllowedTerminalOrigin } from './terminal-security.js';
import { isAllowedRequestHost } from './request-security.js';

const app = new Hono();

const webStaticCandidates = [
  path.resolve(import.meta.dir, '../../../public/dist'),
  path.resolve(process.cwd(), 'public/dist'),
  // Source fallback. Do not use legacy src/web/public/dist: it is ignored and can be stale.
  path.resolve(import.meta.dir, '../public'),
  path.resolve(process.cwd(), 'src/web/public'),
];

export function selectWebStaticDir(candidates: readonly string[]): string {
  return candidates.find((dir) => existsSync(path.join(dir, 'index.html'))) ?? candidates[0];
}

function getWebDistDir(): string {
  return selectWebStaticDir(webStaticCandidates);
}

// Rate limiting: 500 requests per minute for API routes (local tool, be generous)
app.use('/api/*', rateLimit(500, 60 * 1000));

// Serve static files (legacy)
app.use('/static/*', serveStatic({ root: './src/web/public' }));

// Serve React app assets - with path traversal protection
app.get('/assets/*', async (c) => {
  const requestPath = c.req.path;
  const basePath = getWebDistDir();

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
// Auth routes MUST be before usage (/api) — usage has use('*', authMiddleware)
// which would intercept /api/auth/* if mounted first
app.route('/api/auth', auth);
app.route('/api/sessions', sessions);
app.route('/api/sessions', recovery);
app.route('/api/projects', projects);
app.route('/api/work-items', workItemEvidence);
app.route('/api/work-items', workItems);
app.route('/api', usage);
app.route('/api/memory', memory);
app.route('/api/plans', plans);

// Serve React app index.html for root
app.get('/', async () => {
  const file = Bun.file(path.join(getWebDistDir(), 'index.html'));
  return new Response(file, {
    headers: { 'Content-Type': 'text/html' },
  });
});

// Fallback to index.html for SPA routing
app.get('/*', async (c) => {
  const requestPath = c.req.path;
  // Don't catch API or static asset routes
  if (requestPath.startsWith('/api/') || requestPath.startsWith('/assets/') || requestPath.startsWith('/static/')) {
    return c.notFound();
  }
  const file = Bun.file(path.join(getWebDistDir(), 'index.html'));
  return new Response(file, {
    headers: { 'Content-Type': 'text/html' },
  });
});

// Session state tracking for real-time updates
let previousSessionsState: string = '';
let lastRealtimeFullSyncAt = 0;

async function checkAndBroadcastUpdates() {
  try {
    if (wsClients.size === 0) return; // No clients, skip
    const now = Date.now();
    if (shouldRunRealtimeFullSync(lastRealtimeFullSyncAt, now, REALTIME_FULL_SYNC_INTERVAL_MS)) {
      await syncSessions();
      lastRealtimeFullSyncAt = Date.now();
    }
    const sessions = getAggregatedSessionsBasic();
    const stats = getSessionStats(sessions);

    const currentState = JSON.stringify({
      stats,
      sessions: sessions
        .map(s => ({
          sessionId: s.sessionId,
          client: s.client,
          status: s.status,
          directory: s.directory,
          lastActiveAt: s.lastActiveAt.toISOString(),
          title: s.title,
        }))
        .sort((a, b) => a.sessionId.localeCompare(b.sessionId)),
    });

    // Only broadcast if state changed
    if (currentState !== previousSessionsState) {
      previousSessionsState = currentState;

      broadcast('sessions:update', {
        sessions: serializeBasicSessions(sessions),
        stats,
      });
    }
  } catch (error) {
    logger.error('Failed to check for updates', error);
  }
}

export async function startWebServer(port: number = 3377) {
  runMigrations();

  // Initialize memory service for auto-tracking
  initializeMemoryService();

  // Initialize pricing from LiteLLM
  logger.info('Fetching model pricing from LiteLLM...');
  await initPricing();

  // Initial sync on startup (so database has data for first request)
  logger.info('Running initial session sync...');
  await syncSessions();
  lastRealtimeFullSyncAt = Date.now();

  logger.info(`Starting web server on port ${port}`);

  const terminalConfig = config.get().webTerminal;
  const hostname = process.env.KEEPLINE_HOST || '127.0.0.1';

  const server = Bun.serve<{ type: 'dashboard' | 'terminal' }>({
    hostname,
    port,
    idleTimeout: 255, // max value, prevents cloudflared/proxy timeout
    fetch(req, server) {
      const url = new URL(req.url);
      if (!isAllowedRequestHost(req, hostname, port)) {
        logger.warn('Rejected request with invalid Host', {
          host: req.headers.get('host') ?? '<missing>',
          path: url.pathname,
        });
        return new Response('Forbidden', { status: 403 });
      }

      // Handle WebSocket upgrade - dashboard
      if (url.pathname === '/ws') {
        const token = url.searchParams.get('token');
        if (!token || !verifyToken(token)) {
          return new Response('Unauthorized', { status: 401 });
        }
        const upgraded = server.upgrade(req, { data: { type: 'dashboard' } });
        if (upgraded) return undefined;
        return new Response('WebSocket upgrade failed', { status: 400 });
      }

      // Handle WebSocket upgrade - terminal
      if (url.pathname === '/ws/terminal') {
        const tlsEnabled = Boolean(terminalConfig.tlsCert && terminalConfig.tlsKey);
        if (!isAllowedTerminalOrigin(req, hostname, port, tlsEnabled)) {
          logger.warn('Rejected terminal WebSocket with invalid Origin', {
            origin: req.headers.get('origin') ?? '<missing>',
            host: url.host,
          });
          return new Response('Forbidden', { status: 403 });
        }

        const upgraded = server.upgrade(req, { data: { type: 'terminal' } });
        if (upgraded) return undefined;
        return new Response('WebSocket upgrade failed', { status: 400 });
      }

      // Handle regular HTTP requests via Hono
      return app.fetch(req, { server });
    },
    websocket: {
      idleTimeout: 0, // disable WS idle timeout for long-lived terminal sessions
      perMessageDeflate: false, // required for cloudflared compatibility
      open(ws) {
        const data = (ws as any).data as { type: string } | undefined;
        if (data?.type === 'terminal') {
          terminalWebsocketHandler.open(ws);
        } else {
          websocketHandler.open(ws);
        }
      },
      message(ws, message) {
        const data = (ws as any).data as { type: string } | undefined;
        if (data?.type === 'terminal') {
          terminalWebsocketHandler.message(ws, message);
        } else {
          websocketHandler.message(ws, message);
        }
      },
      close(ws) {
        const data = (ws as any).data as { type: string } | undefined;
        if (data?.type === 'terminal') {
          terminalWebsocketHandler.close(ws);
        } else {
          websocketHandler.close(ws);
        }
      },
    },
    ...(terminalConfig.tlsCert && terminalConfig.tlsKey ? {
      tls: {
        cert: Bun.file(terminalConfig.tlsCert),
        key: Bun.file(terminalConfig.tlsKey),
      },
    } : {}),
  });

  // Start periodic update checker (every 5 seconds)
  setInterval(checkAndBroadcastUpdates, REALTIME_POLL_INTERVAL_MS);

  logger.info(`Web UI available at http://${hostname}:${port}`);
  logger.info(`WebSocket available at ws://${hostname}:${port}/ws`);

  return server;
}

export { app };
