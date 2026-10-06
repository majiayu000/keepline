import { Hono } from 'hono';
import { join, resolve } from 'path';
import { existsSync } from 'fs';
import evidence from './routes/work-item-evidence.js';
import projects from './routes/projects.js';
import status from './routes/status.js';
import ledger, { ledgerSettings } from './routes/ledger.js';
import goals from './routes/goals.js';
import { events } from '../../lib/events.js';
import { broadcast } from './websocket.js';
import { webContentSecurityPolicy } from './request-security.js';

export function mountServiceClient(app: Hono) {
  // The monitoring process stays light until a web client requests these UI routes.
  // Scans and recovery workers retain the existing separate-process boundary.
  const authClient = new Hono(); authClient.all('*',async c => (await import('./routes/auth.js')).default.fetch(c.req.raw,c.env));
  const sessionClient = new Hono(); sessionClient.all('*',async c => {
    if (/\/(?:recover|stop|complete)$/.test(c.req.path)) return (await import('./routes/recovery.js')).default.fetch(c.req.raw,c.env);
    return (await import('./routes/sessions.js')).default.fetch(c.req.raw,c.env);
  });
  const workClient = new Hono(); workClient.all('*',async c => (await import('./routes/work-items.js')).default.fetch(c.req.raw,c.env));
  // Strip the outer prefix before forwarding to the existing sub-apps.
  const forward = (prefix: string,router: Hono) => async (c: import('hono').Context) => {
    const url = new URL(c.req.url); url.pathname = url.pathname.slice(prefix.length) || '/';
    return router.fetch(new Request(url.toString(),c.req.raw),c.env);
  };
  app.all('/api/auth/*',forward('/api/auth',authClient));
  app.all('/api/sessions',forward('/api/sessions',sessionClient)); app.all('/api/sessions/*',forward('/api/sessions',sessionClient));
  app.route('/api/work-items',evidence);
  app.all('/api/work-items',forward('/api/work-items',workClient)); app.all('/api/work-items/*',forward('/api/work-items',workClient));
  app.route('/api/projects',projects); app.route('/api/status',status);
  app.route('/api/ledger',ledger); app.route('/api/goals',goals); app.route('/api/settings/ledger',ledgerSettings);
  const forwardAlert = (payload: Record<string,unknown>) => broadcast('ledger:alert',payload);
  const clear = (payload: { id: string; sessionId: string }) => broadcast('ledger:alert-cleared',payload);
  const update = (payload: { sessionId: string }) => broadcast('ledger:update',payload);
  events.on('ledger:alert',forwardAlert); events.on('ledger:alert-cleared',clear); events.on('ledger:update',update);
  app.get('/*',async c => {
    if (c.req.path.startsWith('/api/')) return c.notFound();
    const root = resolve(process.env.KEEPLINE_WEB_DIST ?? join(import.meta.dir,'../../../public/dist'));
    const requested = resolve(root,`.${decodeURIComponent(c.req.path)}`);
    if (!requested.startsWith(root + '/') && requested !== root) return c.notFound();
    const file = Bun.file(existsSync(requested) && c.req.path.includes('.') ? requested : join(root,'index.html'));
    if (!await file.exists()) return c.notFound();
    return new Response(file,{ headers: { 'Content-Security-Policy': webContentSecurityPolicy(c.req.url) } });
  });
  return () => { events.off('ledger:alert',forwardAlert); events.off('ledger:alert-cleared',clear); events.off('ledger:update',update); };
}
