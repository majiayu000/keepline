import { Hono } from 'hono';
import type { Context } from 'hono';
import { workItemRepository } from '../../../infrastructure/database/index.js';
import {
  isWorkItemKind,
  isWorkItemStatus,
  isWorkItemStatusSource,
  type WorkItem,
  type WorkItemCreateInput,
  type WorkItemFilters,
  type WorkItemKind,
  type WorkItemStatus,
  type WorkItemStatusSource,
  type WorkItemUpdateInput,
} from '../../../domain/work-item/index.js';
import { logger } from '../../../lib/logger.js';
import { authMiddleware } from '../middleware/auth.js';

const app = new Hono();
app.use('*', authMiddleware);

function serializeWorkItem(item: WorkItem) {
  return {
    ...item,
    createdAt: item.createdAt.toISOString(),
    updatedAt: item.updatedAt.toISOString(),
    completedAt: item.completedAt?.toISOString(),
  };
}

function readString(
  data: Record<string, unknown>,
  key: string,
  options: { required?: boolean; maxLength?: number } = {}
): { value?: string; error?: string } {
  const raw = data[key];
  if (raw == null) {
    return options.required ? { error: `${key} is required` } : {};
  }
  if (typeof raw !== 'string') {
    return { error: `${key} must be a string` };
  }
  const value = raw.trim();
  if (options.required && !value) {
    return { error: `${key} is required` };
  }
  if (options.maxLength && value.length > options.maxLength) {
    return { error: `${key} must be ${options.maxLength} characters or fewer` };
  }
  return { value: value || undefined };
}

export async function readJsonObject(c: Context): Promise<{
  data?: Record<string, unknown>;
  response?: Response;
}> {
  let body: unknown;
  try {
    body = await c.req.json();
  } catch {
    return { response: c.json({ success: false, error: 'Invalid JSON body' }, 400) };
  }

  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return { response: c.json({ success: false, error: 'Request body must be an object' }, 400) };
  }

  return { data: body as Record<string, unknown> };
}

function parseKind(value: unknown, fallback?: WorkItemKind): { value?: WorkItemKind; error?: string } {
  if (value == null) return { value: fallback };
  return isWorkItemKind(value)
    ? { value }
    : { error: `Invalid work item kind: ${String(value)}` };
}

function parseStatus(value: unknown, fallback?: WorkItemStatus): {
  value?: WorkItemStatus;
  error?: string;
} {
  if (value == null) return { value: fallback };
  return isWorkItemStatus(value)
    ? { value }
    : { error: `Invalid work item status: ${String(value)}` };
}

function parseStatusSource(value: unknown, fallback?: WorkItemStatusSource): {
  value?: WorkItemStatusSource;
  error?: string;
} {
  if (value == null) return { value: fallback };
  if (!isWorkItemStatusSource(value)) {
    return { error: 'statusSource must be user' };
  }
  if (value !== 'user') {
    return {
      error: 'accepted_agent_suggestion requires an explicit suggestion acceptance endpoint',
    };
  }
  return { value };
}

app.get('/', (c) => {
  try {
    const filters: WorkItemFilters = {};
    const statuses = c.req.queries('status') ?? [];
    if (statuses.length > 0) {
      const parsed: WorkItemStatus[] = [];
      for (const status of statuses) {
        if (!isWorkItemStatus(status)) {
          return c.json({ success: false, error: `Invalid work item status: ${status}` }, 400);
        }
        parsed.push(status);
      }
      filters.status = parsed;
    }

    const kind = c.req.query('kind');
    if (kind) {
      if (!isWorkItemKind(kind)) {
        return c.json({ success: false, error: `Invalid work item kind: ${kind}` }, 400);
      }
      filters.kind = kind;
    }

    const projectRoot = c.req.query('projectRoot');
    if (projectRoot) {
      filters.projectRoot = projectRoot;
    }
    filters.includeArchived = c.req.query('includeArchived') === 'true';

    const items = workItemRepository.findAll(filters);
    return c.json({
      success: true,
      data: {
        items: items.map(serializeWorkItem),
        stats: workItemRepository.getOverviewStats(),
      },
    });
  } catch (error) {
    logger.error('Failed to list work items', error);
    return c.json({ success: false, error: 'Failed to list work items' }, 500);
  }
});

app.get('/:id', (c) => {
  try {
    const item = workItemRepository.findById(c.req.param('id'));
    if (!item) {
      return c.json({ success: false, error: 'Work item not found' }, 404);
    }
    return c.json({ success: true, data: { item: serializeWorkItem(item) } });
  } catch (error) {
    logger.error('Failed to get work item', error);
    return c.json({ success: false, error: 'Failed to get work item' }, 500);
  }
});

app.post('/', async (c) => {
  const parsedBody = await readJsonObject(c);
  if (parsedBody.response) return parsedBody.response;
  const data = parsedBody.data!;

  const title = readString(data, 'title', { required: true, maxLength: 200 });
  if (title.error) return c.json({ success: false, error: title.error }, 400);
  const body = readString(data, 'body', { maxLength: 10000 });
  if (body.error) return c.json({ success: false, error: body.error }, 400);
  const projectRoot = readString(data, 'projectRoot', { maxLength: 2048 });
  if (projectRoot.error) return c.json({ success: false, error: projectRoot.error }, 400);
  const area = readString(data, 'area', { maxLength: 120 });
  if (area.error) return c.json({ success: false, error: area.error }, 400);

  const kind = parseKind(data.kind, 'todo');
  if (kind.error) return c.json({ success: false, error: kind.error }, 400);
  const status = parseStatus(data.status, 'inbox');
  if (status.error) return c.json({ success: false, error: status.error }, 400);
  const statusSource = parseStatusSource(data.statusSource, 'user');
  if (statusSource.error) return c.json({ success: false, error: statusSource.error }, 400);

  try {
    const item = workItemRepository.create({
      title: title.value!,
      body: body.value,
      projectRoot: projectRoot.value,
      area: area.value,
      kind: kind.value,
      status: status.value,
      statusSource: statusSource.value,
    } satisfies WorkItemCreateInput);

    return c.json({ success: true, data: { item: serializeWorkItem(item) } }, 201);
  } catch (error) {
    logger.error('Failed to create work item', error);
    return c.json({ success: false, error: 'Failed to create work item' }, 500);
  }
});

app.patch('/:id', async (c) => {
  const parsedBody = await readJsonObject(c);
  if (parsedBody.response) return parsedBody.response;
  const data = parsedBody.data!;

  const input: WorkItemUpdateInput = {};
  const title = readString(data, 'title', { maxLength: 200 });
  if (title.error) return c.json({ success: false, error: title.error }, 400);
  if ('title' in data) {
    if (!title.value) return c.json({ success: false, error: 'title is required' }, 400);
    input.title = title.value;
  }

  const body = readString(data, 'body', { maxLength: 10000 });
  if (body.error) return c.json({ success: false, error: body.error }, 400);
  if ('body' in data) input.body = body.value ?? null;

  const projectRoot = readString(data, 'projectRoot', { maxLength: 2048 });
  if (projectRoot.error) return c.json({ success: false, error: projectRoot.error }, 400);
  if ('projectRoot' in data) input.projectRoot = projectRoot.value ?? null;

  const area = readString(data, 'area', { maxLength: 120 });
  if (area.error) return c.json({ success: false, error: area.error }, 400);
  if ('area' in data) input.area = area.value ?? null;

  const kind = parseKind(data.kind);
  if (kind.error) return c.json({ success: false, error: kind.error }, 400);
  if (kind.value) input.kind = kind.value;

  const status = parseStatus(data.status);
  if (status.error) return c.json({ success: false, error: status.error }, 400);
  if (status.value) input.status = status.value;

  const statusSource = parseStatusSource(data.statusSource, status.value ? 'user' : undefined);
  if (statusSource.error) return c.json({ success: false, error: statusSource.error }, 400);
  if (statusSource.value) input.statusSource = statusSource.value;

  try {
    const item = workItemRepository.update(c.req.param('id'), input);
    if (!item) {
      return c.json({ success: false, error: 'Work item not found' }, 404);
    }
    return c.json({ success: true, data: { item: serializeWorkItem(item) } });
  } catch (error) {
    logger.error('Failed to update work item', error);
    return c.json({ success: false, error: 'Failed to update work item' }, 500);
  }
});

app.delete('/:id', (c) => {
  try {
    const deleted = workItemRepository.delete(c.req.param('id'));
    if (!deleted) {
      return c.json({ success: false, error: 'Work item not found' }, 404);
    }
    return c.json({ success: true });
  } catch (error) {
    logger.error('Failed to delete work item', error);
    return c.json({ success: false, error: 'Failed to delete work item' }, 500);
  }
});

export default app;
