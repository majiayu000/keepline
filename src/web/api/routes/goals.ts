import { readJsonObject } from '../../../local-api/http.js';
import { taskDispatchService } from '../../../services/task-dispatch.service.js';
import { Hono } from 'hono';
import { authMiddleware } from '../middleware/auth.js';
import { goalRows, todoRows, completeTodo } from '../../../services/ledger/goals.js';
import { LedgerInputError } from '../../../services/ledger/service.js';
import { logger } from '../../../lib/logger.js';
const app = new Hono(); app.use('*',authMiddleware);
app.onError((error,c) => {
  if (error instanceof LedgerInputError) return c.json({ success: false,error: error.message },400);
  logger.error('Goals request failed',error); return c.json({ success: false,error: 'Goals request failed' },500);
});
app.get('/',c => c.json({ success: true,data: goalRows(c.req.query('area'),undefined,c.req.query('projectMap')) }));
app.get('/todos',c => c.json({ success: true,data: todoRows() }));
app.post('/todos/:id/complete',c => c.json({ success: true,data: completeTodo(c.req.param('id')) }));
app.post('/todos/:id/dispatch',async c => {
  const body = await readJsonObject(c); if (body.response) return body.response;
  const data = body.data!;
  if (!['codex','claude-code'].includes(String(data.runtimeId)) || typeof data.cwd !== 'string' || typeof data.prompt !== 'string' || typeof data.idempotencyKey !== 'string') throw new LedgerInputError('Invalid dispatch');
  const dispatch = await taskDispatchService.dispatch(c.req.param('id'),{ runtimeId: data.runtimeId as 'codex' | 'claude-code',cwd: data.cwd,prompt: data.prompt,idempotencyKey: data.idempotencyKey });
  return c.json({ success: true,data: dispatch },202);
});
export default app;
