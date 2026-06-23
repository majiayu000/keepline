import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import projects from '../web/api/routes/projects.js';
import { setupUser } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';

describe('Projects Route Contract', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('fields=basic returns project summaries grouped by directory', async () => {
    sessionRepository.upsert({
      sessionId: 'project-route-1',
      client: 'claude',
      directory: '/tmp/keepline-project-route-a',
      status: 'running',
      title: 'Route project A',
      initialPrompt: 'A',
      lastActiveAt: new Date('2026-04-13T10:00:05.000Z'),
      toolCount: 2,
      messageCount: 3,
      usageStats: {
        totalInputTokens: 100,
        totalOutputTokens: 50,
        totalTokens: 150,
        totalCost: 0.02,
        apiCalls: 2,
      },
    });
    sessionRepository.upsert({
      sessionId: 'project-route-2',
      client: 'codex',
      directory: '/tmp/keepline-project-route-b',
      status: 'waiting',
      title: 'Route project B',
      initialPrompt: 'B',
      lastActiveAt: new Date('2026-04-13T10:01:05.000Z'),
      toolCount: 1,
      messageCount: 4,
    });

    const { token } = await setupUser('projects-route-user', 'password123');
    const response = await projects.fetch(new Request(
      'http://localhost/?fields=basic',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(200);

    const body = await response.json() as {
      success: boolean;
      data: {
        projects: Array<{
          id: string;
          rootPath: string;
          clientCounts: { claude: number; codex: number; unknown: number };
          runtimeCounts: { 'claude-code': number; codex: number; unknown: number };
          totalUsage?: { totalCost: number; totalTokens: number; apiCalls: number };
          sessions?: Array<{ sessionId: string; client: string; runtimeId: string }>;
        }>;
        stats: { total: number; active: number };
      };
    };

    expect(body.success).toBe(true);
    expect(body.data.projects).toHaveLength(2);
    expect(body.data.stats).toMatchObject({ total: 2 });
    expect(body.data.projects.map(project => project.id)).not.toContain('unknown');

    const codexProject = body.data.projects.find(project => project.rootPath === '/tmp/keepline-project-route-b');
    expect(codexProject).toBeDefined();
    expect(codexProject!.clientCounts.codex).toBe(1);
    expect(codexProject!.runtimeCounts.codex).toBe(1);
    expect(codexProject).not.toHaveProperty('sessions');

    const claudeProject = body.data.projects.find(project => project.rootPath === '/tmp/keepline-project-route-a');
    expect(claudeProject).toBeDefined();
    expect(claudeProject).not.toHaveProperty('sessions');
    expect(claudeProject!.totalUsage).toMatchObject({
      totalCost: 0.02,
      totalTokens: 150,
      apiCalls: 2,
    });
  });

  test('fields=full returns full nested sessions', async () => {
    sessionRepository.upsert({
      sessionId: 'project-route-full-1',
      client: 'claude',
      directory: '/tmp/keepline-project-route-full',
      status: 'running',
      title: 'Full route project',
      initialPrompt: 'Full prompt',
      lastMessage: 'Detailed latest response',
      lastActiveAt: new Date('2026-04-13T10:00:05.000Z'),
      toolCount: 2,
      messageCount: 3,
      usageStats: {
        totalInputTokens: 100,
        totalOutputTokens: 50,
        totalTokens: 150,
        totalCost: 0.02,
        apiCalls: 2,
      },
      toolCalls: [{
        name: 'Read',
        input: { file_path: 'src/index.ts' },
        timestamp: '2026-04-13T10:00:00.000Z',
      }],
    });

    const { token } = await setupUser('projects-route-full-user', 'password123');
    const response = await projects.fetch(new Request(
      'http://localhost/?fields=full',
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(200);

    const body = await response.json() as {
      success: boolean;
      data: {
        projects: Array<{
          rootPath: string;
          sessions?: Array<{
            sessionId: string;
            runtimeId?: string;
            initialPrompt?: string;
            lastMessage?: string;
            usageStats?: { totalCost: number; totalTokens: number; apiCalls: number };
            toolCalls?: Array<{ name: string }>;
          }>;
        }>;
      };
    };

    expect(body.success).toBe(true);
    const project = body.data.projects.find(item => item.rootPath === '/tmp/keepline-project-route-full');
    expect(project).toBeDefined();
    expect(project!.sessions).toHaveLength(1);
    expect(project!.sessions![0]).toMatchObject({
      sessionId: 'project-route-full-1',
      runtimeId: 'claude-code',
      initialPrompt: 'Full prompt',
      lastMessage: 'Detailed latest response',
      usageStats: {
        totalCost: 0.02,
        totalTokens: 150,
        apiCalls: 2,
      },
    });
    expect(project!.sessions![0].toolCalls).toEqual([
      expect.objectContaining({ name: 'Read' }),
    ]);
  });
});
