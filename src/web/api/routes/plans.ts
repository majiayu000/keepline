/**
 * Plans Routes
 *
 * Handles Claude Code plan file operations
 */

import { Hono } from 'hono';
import {
  scanPlansDirectory,
  getPlanSummaries,
  getPlanById,
  getPlanStats,
} from '../../../adapters/claude/plans/index.js';
import { logger } from '../../../lib/logger.js';

const app = new Hono();

// GET /api/plans - List all plans
app.get('/', (c) => {
  try {
    const plans = scanPlansDirectory();

    return c.json({
      success: true,
      data: plans.map(p => ({
        id: p.id,
        title: p.title,
        modifiedAt: p.modifiedAt.toISOString(),
        createdAt: p.createdAt.toISOString(),
        stats: p.stats,
        phases: p.phases.map(phase => ({
          name: phase.name,
          title: phase.title,
          completedCount: phase.completedCount,
          totalCount: phase.totalCount,
        })),
      })),
    });
  } catch (error) {
    logger.error('Failed to list plans', error);
    return c.json({ success: false, error: 'Failed to list plans' }, 500);
  }
});

// GET /api/plans/summaries - Get plan summaries
app.get('/summaries', (c) => {
  try {
    const summaries = getPlanSummaries();
    return c.json({
      success: true,
      data: summaries.map(s => ({
        ...s,
        modifiedAt: s.modifiedAt.toISOString(),
      })),
    });
  } catch (error) {
    logger.error('Failed to get plan summaries', error);
    return c.json({ success: false, error: 'Failed to get summaries' }, 500);
  }
});

// GET /api/plans/stats - Get aggregate stats
app.get('/stats', (c) => {
  try {
    const stats = getPlanStats();
    return c.json({
      success: true,
      data: stats,
    });
  } catch (error) {
    logger.error('Failed to get plan stats', error);
    return c.json({ success: false, error: 'Failed to get stats' }, 500);
  }
});

// GET /api/plans/:id - Get a specific plan with full content
app.get('/:id', (c) => {
  const id = c.req.param('id');

  try {
    const plan = getPlanById(id);

    if (!plan) {
      return c.json({ success: false, error: 'Plan not found' }, 404);
    }

    return c.json({
      success: true,
      data: {
        id: plan.id,
        title: plan.title,
        filePath: plan.filePath,
        modifiedAt: plan.modifiedAt.toISOString(),
        createdAt: plan.createdAt.toISOString(),
        content: plan.content,
        phases: plan.phases,
        tasks: plan.tasks,
        stats: plan.stats,
      },
    });
  } catch (error) {
    logger.error('Failed to get plan', error);
    return c.json({ success: false, error: 'Failed to get plan' }, 500);
  }
});

export default app;
