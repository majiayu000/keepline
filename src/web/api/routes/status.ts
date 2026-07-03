import { Hono } from 'hono';
import { getHookAvailability } from '../../../adapters/hook/availability.js';
import { authMiddleware } from '../middleware/auth.js';

const app = new Hono();

app.use('*', authMiddleware);

app.get('/hooks', async (c) => {
  return c.json({
    success: true,
    data: await getHookAvailability(),
  });
});

export default app;
