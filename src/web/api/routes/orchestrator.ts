import { Hono } from 'hono';
import { getAggregatedSessions } from '../../../services/session.aggregator.js';
import {
  buildAttentionOverview,
  MAX_ATTENTION_LIMIT,
  type AttentionOverview,
  type AttentionQueueItem,
} from '../../../services/attention.prioritizer.js';
import { logger } from '../../../lib/logger.js';
import { authMiddleware } from '../middleware/auth.js';

const app = new Hono();
app.use('*', authMiddleware);

app.get('/overview', (c) => {
  const limitResult = parsePositiveNumber(c.req.query('limit'), 'limit');
  if (limitResult.error) {
    return c.json({ success: false, error: limitResult.error }, 400);
  }
  const highCostResult = parsePositiveNumber(
    c.req.query('highCostThreshold'),
    'highCostThreshold'
  );
  if (highCostResult.error) {
    return c.json({ success: false, error: highCostResult.error }, 400);
  }
  const staleHoursResult = parsePositiveNumber(c.req.query('staleHours'), 'staleHours');
  if (staleHoursResult.error) {
    return c.json({ success: false, error: staleHoursResult.error }, 400);
  }

  try {
    const overview = buildAttentionOverview(getAggregatedSessions(), {
      includeCompleted: c.req.query('includeCompleted') === 'true',
      limit: limitResult.value,
      highCostThreshold: highCostResult.value,
      staleHours: staleHoursResult.value,
    });

    return c.json({
      success: true,
      data: serializeOverview(overview),
    });
  } catch (error) {
    logger.error('Failed to build orchestrator overview', error);
    return c.json({ success: false, error: 'Failed to build orchestrator overview' }, 500);
  }
});

function parsePositiveNumber(
  value: string | undefined,
  name: string
): { value?: number; error?: string } {
  if (value == null || value.trim() === '') return {};
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return { error: `${name} must be a positive number` };
  }
  if (name === 'limit' && parsed > MAX_ATTENTION_LIMIT) {
    return { error: `limit must be ${MAX_ATTENTION_LIMIT} or less` };
  }
  return { value: parsed };
}

function serializeOverview(overview: AttentionOverview) {
  return {
    generatedAt: overview.generatedAt.toISOString(),
    items: overview.items.map(serializeItem),
    stats: overview.stats,
  };
}

function serializeItem(item: AttentionQueueItem) {
  return {
    ...item,
    lastActiveAt: item.lastActiveAt.toISOString(),
  };
}

export default app;
