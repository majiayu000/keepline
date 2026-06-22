import { Hono } from 'hono';
import {
  getAggregatedSessions,
  getAggregatedSessionsBasic,
} from '../../../services/session.aggregator.js';
import {
  aggregateProjectSummaries,
  getProjectOverviewStats,
} from '../../../services/project.aggregator.js';
import { logger } from '../../../lib/logger.js';
import { authMiddleware } from '../middleware/auth.js';
import { serializeProjectSummaries } from '../project-response.js';

const app = new Hono();
app.use('*', authMiddleware);

app.get('/', async (c) => {
  try {
    const fields = c.req.query('fields') || 'basic';
    const sessions = fields === 'full'
      ? getAggregatedSessions()
      : getAggregatedSessionsBasic();
    const projects = aggregateProjectSummaries(sessions);

    return c.json({
      success: true,
      data: {
        projects: serializeProjectSummaries(projects, { includeSessions: fields === 'full' }),
        stats: getProjectOverviewStats(projects),
      },
    });
  } catch (error) {
    logger.error('Failed to get projects', error);
    return c.json({ success: false, error: 'Failed to get projects' }, 500);
  }
});

export default app;
