import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import workItems from '../web/api/routes/work-items.js';
import { setupUser } from '../services/auth.service.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';

async function authedRequest(
  token: string,
  path: string,
  options: RequestInit = {}
): Promise<Response> {
  const headers = new Headers(options.headers);
  headers.set('Authorization', `Bearer ${token}`);
  if (options.body && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  return workItems.fetch(new Request(`http://localhost${path}`, {
    ...options,
    headers,
  }));
}

describe('Work item routes', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('captures a work item without a project', async () => {
    const { token } = await setupUser('work-item-capture-user', 'password123');
    const response = await authedRequest(token, '/', {
      method: 'POST',
      body: JSON.stringify({
        title: 'Triage loose inbox item',
        kind: 'todo',
      }),
    });

    expect(response.status).toBe(201);
    const body = await response.json() as {
      success: boolean;
      data: {
        item: {
          id: string;
          title: string;
          kind: string;
          status: string;
          statusSource: string;
          projectRoot?: string;
        };
      };
    };

    expect(body.success).toBe(true);
    expect(body.data.item).toMatchObject({
      title: 'Triage loose inbox item',
      kind: 'todo',
      status: 'inbox',
      statusSource: 'user',
    });
    expect(body.data.item.projectRoot).toBeUndefined();
  });

  test('rejects suggestion-sourced formal status changes through generic CRUD', async () => {
    const { token } = await setupUser('work-item-agent-user', 'password123');
    const createResponse = await authedRequest(token, '/', {
      method: 'POST',
      body: JSON.stringify({ title: 'Do not silently close', kind: 'idea' }),
    });
    const createBody = await createResponse.json() as {
      data: { item: { id: string } };
    };

    const rejected = await authedRequest(token, `/${createBody.data.item.id}`, {
      method: 'PATCH',
      body: JSON.stringify({
        status: 'done',
        statusSource: 'heuristic_suggestion',
      }),
    });

    expect(rejected.status).toBe(400);
    const rejectedBody = await rejected.json() as { success: boolean; error: string };
    expect(rejectedBody.success).toBe(false);
    expect(rejectedBody.error).toContain('statusSource');

    const acceptedWithoutArtifact = await authedRequest(token, `/${createBody.data.item.id}`, {
      method: 'PATCH',
      body: JSON.stringify({
        status: 'planned',
        statusSource: 'accepted_agent_suggestion',
      }),
    });

    expect(acceptedWithoutArtifact.status).toBe(400);
    const acceptedBody = await acceptedWithoutArtifact.json() as {
      success: boolean;
      error: string;
    };
    expect(acceptedBody.success).toBe(false);
    expect(acceptedBody.error).toContain('explicit suggestion acceptance endpoint');

    const getResponse = await authedRequest(token, `/${createBody.data.item.id}`);
    const getBody = await getResponse.json() as {
      data: { item: { status: string; statusSource: string; completedAt?: string } };
    };
    expect(getBody.data.item).toMatchObject({
      status: 'inbox',
      statusSource: 'user',
    });
    expect(getBody.data.item.completedAt).toBeUndefined();
  });

  test('allows user actions to change formal status explicitly', async () => {
    const { token } = await setupUser('work-item-accepted-user', 'password123');
    const createResponse = await authedRequest(token, '/', {
      method: 'POST',
      body: JSON.stringify({ title: 'Plan manually', kind: 'todo' }),
    });
    const createBody = await createResponse.json() as {
      data: { item: { id: string } };
    };

    const updateResponse = await authedRequest(token, `/${createBody.data.item.id}`, {
      method: 'PATCH',
      body: JSON.stringify({
        status: 'planned',
        statusSource: 'user',
      }),
    });

    expect(updateResponse.status).toBe(200);
    const updateBody = await updateResponse.json() as {
      success: boolean;
      data: { item: { status: string; statusSource: string } };
    };
    expect(updateBody.success).toBe(true);
    expect(updateBody.data.item).toMatchObject({
      status: 'planned',
      statusSource: 'user',
    });
  });

  test('lists items with overview stats and supports deletion', async () => {
    const { token } = await setupUser('work-item-list-user', 'password123');
    const createTodo = await authedRequest(token, '/', {
      method: 'POST',
      body: JSON.stringify({ title: 'Todo item', kind: 'todo' }),
    });
    await authedRequest(token, '/', {
      method: 'POST',
      body: JSON.stringify({ title: 'Idea item', kind: 'idea', area: 'Product' }),
    });
    const todoBody = await createTodo.json() as { data: { item: { id: string } } };

    const listResponse = await authedRequest(token, '/');
    expect(listResponse.status).toBe(200);
    const listBody = await listResponse.json() as {
      success: boolean;
      data: {
        items: Array<{ title: string; area?: string }>;
        stats: { total: number; inbox: number; todo: number; idea: number };
      };
    };
    expect(listBody.success).toBe(true);
    expect(listBody.data.items).toHaveLength(2);
    expect(listBody.data.items.map((item) => item.title)).toContain('Idea item');
    expect(listBody.data.items.find((item) => item.title === 'Idea item')?.area).toBe('Product');
    expect(listBody.data.stats).toMatchObject({
      total: 2,
      inbox: 2,
      todo: 1,
      idea: 1,
    });

    const deleteResponse = await authedRequest(token, `/${todoBody.data.item.id}`, {
      method: 'DELETE',
    });
    expect(deleteResponse.status).toBe(200);

    const afterDelete = await authedRequest(token, '/');
    const afterDeleteBody = await afterDelete.json() as {
      data: { items: Array<{ title: string }> };
    };
    expect(afterDeleteBody.data.items.map((item) => item.title)).not.toContain('Todo item');
  });
});
