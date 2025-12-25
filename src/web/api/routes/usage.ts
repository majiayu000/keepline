/**
 * Usage Routes
 *
 * Handles usage analytics and quota endpoints
 */

import { Hono } from 'hono';
import { logger } from '../../../lib/logger.js';
import { getCostPrediction, getCostForDateRange } from '../../../services/cost.predictor.js';

const app = new Hono();

// GET /api/quota - Get Claude Code quota/rate limits from OAuth API
app.get('/quota', async (c) => {
  try {
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
