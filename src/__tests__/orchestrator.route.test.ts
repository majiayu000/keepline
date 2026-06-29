import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import orchestrator from '../web/api/routes/orchestrator.js';
import { app } from '../web/api/server.js';
import { setupUser } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

describe('Orchestrator Route Contract', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('overview returns serialized attention queue with auth', async () => {
    sessionRepository.upsert({
      sessionId: 'orchestrator-route-cost',
      client: 'codex',
      directory: '/tmp/keepline-orchestrator',
      status: 'running',
      title: 'Expensive local run',
      initialPrompt: 'Track cost',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
      toolCount: 2,
      messageCount: 4,
      usageStats: {
        totalInputTokens: 100,
        totalOutputTokens: 50,
        totalTokens: 150,
        totalCost: 5,
        apiCalls: 2,
      },
    });

    const { token } = await setupUser('orchestrator-route-user', 'password123');
    const response = await orchestrator.fetch(new Request(
      'http://localhost/overview?limit=10&highCostThreshold=1',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(200);

    const body = await response.json() as {
      success: boolean;
      data: {
        generatedAt: string;
        items: Array<{
          sessionId: string;
          lastActiveAt: string;
          reasons: Array<{ code: string }>;
        }>;
        stats: { totalCandidates: number };
      };
    };

    expect(body.success).toBe(true);
    expect(body.data.generatedAt).toEqual(expect.any(String));
    expect(body.data.stats.totalCandidates).toBe(1);
    expect(body.data.items).toHaveLength(1);
    expect(body.data.items[0]).toMatchObject({
      sessionId: 'orchestrator-route-cost',
      lastActiveAt: '2026-06-29T10:00:00.000Z',
    });
    expect(body.data.items[0].reasons.map((reason) => reason.code)).toContain('high_cost');
  });

  test('server mounts the orchestrator overview route', async () => {
    sessionRepository.upsert({
      sessionId: 'mounted-orchestrator-route',
      client: 'claude',
      directory: '/tmp/keepline-mounted-orchestrator',
      status: 'running',
      title: 'Mounted route',
      initialPrompt: 'Mounted',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
      toolCount: 1,
      messageCount: 1,
    });

    const { token } = await setupUser('mounted-orchestrator-user', 'password123');
    const response = await app.fetch(new Request(
      'http://localhost/api/orchestrator/overview?limit=1',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(200);
    const body = await response.json() as {
      success: boolean;
      data: { items: Array<{ sessionId: string }> };
    };
    expect(body.success).toBe(true);
    expect(body.data.items).toHaveLength(1);
    expect(body.data.items[0].sessionId).toBe('mounted-orchestrator-route');
  });

  test('rejects invalid numeric query values', async () => {
    const { token } = await setupUser('orchestrator-invalid-user', 'password123');
    const response = await orchestrator.fetch(new Request(
      'http://localhost/overview?limit=abc',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(400);
    const body = await response.json() as { success: boolean; error: string };
    expect(body.success).toBe(false);
    expect(body.error).toBe('limit must be a positive number');
  });

  test('requires authentication', async () => {
    const response = await orchestrator.fetch(new Request('http://localhost/overview'));

    expect(response.status).toBe(401);
  });
});
