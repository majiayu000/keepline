import { Hono } from 'hono';
import { getHookAvailability } from '../../../adapters/hook/availability.js';

const app = new Hono();

app.get('/hooks', (c) => {
  return c.json({
    success: true,
    data: getHookAvailability(),
  });
});

export default app;

