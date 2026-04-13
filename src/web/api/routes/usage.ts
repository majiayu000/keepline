/**
 * Usage Routes
 *
 * Handles usage analytics and quota endpoints
 */

import { Hono } from 'hono';
import { join } from 'path';
import { logger } from '../../../lib/logger.js';
import { CLAUDE_HUB_HOME } from '../../../lib/paths.js';
import { getCostPrediction, getCostForDateRange } from '../../../services/cost.predictor.js';
import { authMiddleware } from '../middleware/auth.js';
import { ExpiringCache } from '../expiring-cache.js';

const app = new Hono();
app.use('*', authMiddleware);

const CODEX_CLIENT_ID = 'app_EMoamEEZ73f0CkXaXp7hrann';
const CODEX_USAGE_URL = 'https://chatgpt.com/backend-api/wham/usage';
const CODEX_REFRESH_URL = 'https://auth.openai.com/oauth/token';

const DEFAULT_CLIENTS_FILE = join(CLAUDE_HUB_HOME, 'clients.json');
const QUOTA_CACHE_TTL_MS = 30_000;
const USAGE_CACHE_TTL_MS = 60_000;

const quotaCache = new ExpiringCache<Record<string, unknown>>();
const codexQuotaCache = new ExpiringCache<Record<string, unknown>>();
const usageCache = new ExpiringCache<unknown>();

const DEFAULT_CODEX_AUTH_FILE = (() => {
  const homeDir = process.env.HOME;
  if (!homeDir) return null;
  return join(homeDir, '.codex', 'auth.json');
})();

type CodexAuthFile = {
  OPENAI_API_KEY?: string;
  tokens?: {
    id_token?: string;
    access_token?: string;
    refresh_token?: string;
    account_id?: string;
  };
  last_refresh?: string;
};

function decodeJwtPayload(token: string): Record<string, unknown> | null {
  const segments = token.split('.');
  if (segments.length < 2) return null;
  let base64 = segments[1].replace(/-/g, '+').replace(/_/g, '/');
  const padLength = (4 - (base64.length % 4)) % 4;
  base64 += '='.repeat(padLength);
  try {
    const payload = Buffer.from(base64, 'base64').toString('utf8');
    return JSON.parse(payload) as Record<string, unknown>;
  } catch {
    return null;
  }
}

function isTokenExpired(token: string): boolean {
  const payload = decodeJwtPayload(token);
  const exp = typeof payload?.exp === 'number' ? payload.exp : null;
  if (!exp) return true;
  const expiryMs = exp * 1000;
  return expiryMs < Date.now() + 60_000;
}

async function refreshCodexAccessToken(refreshToken: string): Promise<string | null> {
  const response = await fetch(CODEX_REFRESH_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify({
      grant_type: 'refresh_token',
      refresh_token: refreshToken,
      client_id: CODEX_CLIENT_ID,
    }),
  });

  if (!response.ok) {
    return null;
  }

  const data = await response.json() as { access_token?: string };
  return data.access_token ?? null;
}

function parseCodexUsageResponse(json: Record<string, unknown>) {
  const rateLimit = typeof json.rate_limit === 'object' && json.rate_limit ? json.rate_limit as Record<string, unknown> : {};
  const primaryWindow = typeof rateLimit.primary_window === 'object' && rateLimit.primary_window ? rateLimit.primary_window as Record<string, unknown> : {};
  const secondaryWindow = typeof rateLimit.secondary_window === 'object' && rateLimit.secondary_window ? rateLimit.secondary_window as Record<string, unknown> : {};

  const sessionUsed = typeof primaryWindow.used_percent === 'number' ? primaryWindow.used_percent : 0;
  const weeklyUsed = typeof secondaryWindow.used_percent === 'number' ? secondaryWindow.used_percent : 0;
  const sessionResetAt = typeof primaryWindow.reset_at === 'number'
    ? new Date(primaryWindow.reset_at * 1000).toISOString()
    : null;
  const weeklyResetAt = typeof secondaryWindow.reset_at === 'number'
    ? new Date(secondaryWindow.reset_at * 1000).toISOString()
    : null;

  return {
    session: {
      utilization: sessionUsed,
      resets_at: sessionResetAt,
    },
    weekly: {
      utilization: weeklyUsed,
      resets_at: weeklyResetAt,
    },
    limit_reached: typeof rateLimit.limit_reached === 'boolean' ? rateLimit.limit_reached : undefined,
    plan_type: typeof json.plan_type === 'string' ? json.plan_type : undefined,
  };
}

function normalizeClients(raw: unknown) {
  const payload = Array.isArray(raw) ? raw : (raw && typeof raw === 'object' && 'clients' in raw ? (raw as { clients?: unknown }).clients : []);
  if (!Array.isArray(payload)) return [];

  return payload
    .filter((client): client is Record<string, unknown> => !!client && typeof client === 'object')
    .map((client) => ({
      id: typeof client.id === 'string' ? client.id : '',
      name: typeof client.name === 'string' ? client.name : '',
      kind: typeof client.kind === 'string' ? client.kind : undefined,
      status: typeof client.status === 'string' ? client.status : undefined,
      note: typeof client.note === 'string' ? client.note : undefined,
      quota_windows: Array.isArray(client.quota_windows)
        ? client.quota_windows
            .filter((window): window is Record<string, unknown> => !!window && typeof window === 'object')
            .map((window) => ({
              id: typeof window.id === 'string' ? window.id : undefined,
              label: typeof window.label === 'string' ? window.label : '',
              utilization: typeof window.utilization === 'number' ? window.utilization : 0,
              resets_at: typeof window.resets_at === 'string' || window.resets_at === null ? window.resets_at : null,
            }))
        : undefined,
    }))
    .filter((client) => client.id && client.name);
}

// GET /api/clients - Load optional client definitions for multi-client quota display
app.get('/clients', async (c) => {
  try {
    const clientsFile = process.env.CLAUDE_HUB_CLIENTS_FILE || DEFAULT_CLIENTS_FILE;
    if (!clientsFile) {
      return c.json({ success: true, data: { clients: [], source_path: null } });
    }

    const file = Bun.file(clientsFile);
    if (!await file.exists()) {
      return c.json({ success: true, data: { clients: [], source_path: clientsFile } });
    }

    const contents = await file.text();
    const parsed = JSON.parse(contents);
    const clients = normalizeClients(parsed);

    return c.json({ success: true, data: { clients, source_path: clientsFile } });
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.error('Failed to load clients', { message: errorMessage });
    return c.json({ success: false, error: 'Failed to load clients' }, 500);
  }
});

// GET /api/codex/quota - Get Codex CLI quota from ~/.codex/auth.json
app.get('/codex/quota', async (c) => {
  try {
    const cacheKey = process.env.CODEX_AUTH_PATH || DEFAULT_CODEX_AUTH_FILE || 'default';
    const cached = codexQuotaCache.get(cacheKey);
    if (cached) {
      return c.json({ success: true, data: cached });
    }

    const authPath = process.env.CODEX_AUTH_PATH || DEFAULT_CODEX_AUTH_FILE;
    if (!authPath) {
      return c.json({ success: false, error: 'Codex auth path not available' }, 500);
    }

    const authFile = Bun.file(authPath);
    if (!await authFile.exists()) {
      return c.json({ success: false, error: 'Codex auth file not found' }, 404);
    }

    const raw = await authFile.text();
    const authData = JSON.parse(raw) as CodexAuthFile;
    const tokens = authData.tokens;
    if (!tokens?.access_token) {
      return c.json({ success: false, error: 'Codex access token not found' }, 401);
    }

    let accessToken = tokens.access_token;
    if (isTokenExpired(accessToken) && tokens.refresh_token) {
      const refreshed = await refreshCodexAccessToken(tokens.refresh_token);
      if (refreshed) {
        accessToken = refreshed;
      }
    }

    const response = await fetch(CODEX_USAGE_URL, {
      headers: {
        Authorization: `Bearer ${accessToken}`,
        Accept: 'application/json',
      },
    });

    if (!response.ok) {
      const text = await response.text();
      logger.error('Codex usage API failed', { status: response.status, body: text });
      return c.json({ success: false, error: 'Failed to fetch Codex quota' }, 502);
    }

    const json = await response.json() as Record<string, unknown>;
    const payload = parseCodexUsageResponse(json);
    const idToken = tokens.id_token;
    const claims = idToken ? decodeJwtPayload(idToken) : null;
    const email = typeof claims?.email === 'string' ? claims.email : undefined;

    const data = {
      ...payload,
      email,
    };
    codexQuotaCache.set(cacheKey, data, QUOTA_CACHE_TTL_MS);

    return c.json({ success: true, data });
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.error('Failed to get Codex quota', { message: errorMessage });
    return c.json({ success: false, error: 'Failed to get Codex quota' }, 500);
  }
});

// GET /api/quota - Get Claude Code quota/rate limits from OAuth API
app.get('/quota', async (c) => {
  try {
    const cached = quotaCache.get('claude');
    if (cached) {
      return c.json({ success: true, data: cached });
    }

    // Try multiple possible credential names in macOS Keychain
    const credentialNames = [
      'Claude Code-credentials',
      'claude-credentials',
      'Claude-credentials',
      'claudecode-credentials'
    ];

    let output = '';
    let found = false;

    for (const credName of credentialNames) {
      const proc = Bun.spawn(['security', 'find-generic-password', '-s', credName, '-w'], {
        stdout: 'pipe',
        stderr: 'pipe',
      });

      output = await new Response(proc.stdout).text();
      const exitCode = await proc.exited;

      if (exitCode === 0 && output.trim()) {
        found = true;
        logger.info(`Found credentials with name: ${credName}`);
        break;
      }
    }

    if (!found || !output.trim()) {
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
      // Return more detailed error info
      const errorType = data.error.type || 'unknown';
      const errorMsg = data.error.message || 'Quota API error';
      return c.json({
        success: false,
        error: `${errorMsg} (${errorType})`,
        details: 'Token may be expired. Try logging out and back into Claude Code.'
      }, 403);
    }

    quotaCache.set('claude', data, QUOTA_CACHE_TTL_MS);
    return c.json({ success: true, data });
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.error('Failed to get quota', { message: errorMessage });
    return c.json({ success: false, error: 'Failed to get quota data' }, 500);
  }
});

// GET /api/usage - Get usage analytics from ccusage CLI tool
app.get('/usage', async (c) => {
  try {
    const type = c.req.query('type') || 'daily'; // daily, monthly, weekly, session
    const since = c.req.query('since'); // YYYYMMDD format
    const until = c.req.query('until'); // YYYYMMDD format
    const cacheKey = JSON.stringify({ type, since: since || '', until: until || '' });
    const cached = usageCache.get(cacheKey);
    if (cached) {
      return c.json({ success: true, data: cached });
    }

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
    usageCache.set(cacheKey, data, USAGE_CACHE_TTL_MS);
    return c.json({ success: true, data });
  } catch (error) {
    logger.error('Failed to get usage data', error);
    return c.json({ success: false, error: 'Failed to get usage data' }, 500);
  }
});

// GET /api/usage/prediction - Get cost prediction and analytics
app.get('/usage/prediction', async (c) => {
  try {
    const prediction = await getCostPrediction();
    return c.json({ success: true, data: prediction });
  } catch (error) {
    logger.error('Failed to get cost prediction', error);
    return c.json({ success: false, error: 'Failed to get cost prediction' }, 500);
  }
});

// GET /api/usage/today - Get today's usage
app.get('/usage/today', async (c) => {
  try {
    const now = new Date();
    const startOfDay = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const endOfDay = new Date(startOfDay.getTime() + 24 * 60 * 60 * 1000 - 1);

    const data = await getCostForDateRange(startOfDay, endOfDay);
    return c.json({ success: true, data });
  } catch (error) {
    logger.error('Failed to get today usage', error);
    return c.json({ success: false, error: 'Failed to get today usage' }, 500);
  }
});

// GET /api/usage/week - Get this week's usage
app.get('/usage/week', async (c) => {
  try {
    const now = new Date();
    const day = now.getDay();
    const diff = now.getDate() - day + (day === 0 ? -6 : 1); // Monday start
    const startOfWeek = new Date(now.getFullYear(), now.getMonth(), diff);
    const endOfWeek = new Date(startOfWeek.getTime() + 7 * 24 * 60 * 60 * 1000 - 1);

    const data = await getCostForDateRange(startOfWeek, endOfWeek);
    return c.json({ success: true, data });
  } catch (error) {
    logger.error('Failed to get week usage', error);
    return c.json({ success: false, error: 'Failed to get week usage' }, 500);
  }
});

// GET /api/usage/month - Get this month's usage
app.get('/usage/month', async (c) => {
  try {
    const now = new Date();
    const startOfMonth = new Date(now.getFullYear(), now.getMonth(), 1);
    const endOfMonth = new Date(now.getFullYear(), now.getMonth() + 1, 0, 23, 59, 59);

    const data = await getCostForDateRange(startOfMonth, endOfMonth);
    return c.json({ success: true, data });
  } catch (error) {
    logger.error('Failed to get month usage', error);
    return c.json({ success: false, error: 'Failed to get month usage' }, 500);
  }
});

// GET /api/usage/range - Get usage for custom date range
app.get('/usage/range', async (c) => {
  try {
    const startParam = c.req.query('start');
    const endParam = c.req.query('end');

    if (!startParam || !endParam) {
      return c.json({ success: false, error: 'start and end parameters required (YYYY-MM-DD)' }, 400);
    }

    // Validate date format
    const dateRegex = /^\d{4}-\d{2}-\d{2}$/;
    if (!dateRegex.test(startParam) || !dateRegex.test(endParam)) {
      return c.json({ success: false, error: 'Invalid date format. Use YYYY-MM-DD' }, 400);
    }

    const startDate = new Date(startParam);
    const endDate = new Date(endParam + 'T23:59:59');

    if (isNaN(startDate.getTime()) || isNaN(endDate.getTime())) {
      return c.json({ success: false, error: 'Invalid date values' }, 400);
    }

    if (startDate > endDate) {
      return c.json({ success: false, error: 'start date must be before end date' }, 400);
    }

    const data = await getCostForDateRange(startDate, endDate);
    return c.json({ success: true, data });
  } catch (error) {
    logger.error('Failed to get range usage', error);
    return c.json({ success: false, error: 'Failed to get range usage' }, 500);
  }
});

export default app;
