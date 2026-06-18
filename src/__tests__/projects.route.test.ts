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
          sessions: Array<{ sessionId: string; client: string }>;
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
    expect(codexProject!.sessions[0].sessionId).toBe('project-route-2');
  });
});
