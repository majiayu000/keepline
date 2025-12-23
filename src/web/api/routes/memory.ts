/**
 * Memory Routes
 *
 * Handles session memory operations for the "relay race" pattern
 */

import { Hono } from 'hono';
import { memoryRepository } from '../../../infrastructure/database/index.js';
import { buildContext, buildMinimalContext } from '../../../domain/memory/index.js';
import type { MemoryUpsertData } from '../../../domain/memory/index.js';
import { logger } from '../../../lib/logger.js';
import { isValidSessionId } from '../middleware/validation.js';

const app = new Hono();

// GET /api/memory - List all session memories
app.get('/', (c) => {
  try {
    const limit = c.req.query('limit');
    const directory = c.req.query('directory');

    let memories;
    if (directory) {
      memories = memoryRepository.findByDirectory(directory);
    } else {
      const limitNum = limit ? parseInt(limit, 10) : 20;
      memories = memoryRepository.findRecent(limitNum);
    }

    return c.json({
      success: true,
      data: memories.map(m => ({
        ...m,
        createdAt: m.createdAt.toISOString(),
        updatedAt: m.updatedAt.toISOString(),
      })),
    });
  } catch (error) {
    logger.error('Failed to list memories', error);
    return c.json({ success: false, error: 'Failed to list memories' }, 500);
  }
});

// GET /api/memory/summaries - Get memory summaries
app.get('/summaries', (c) => {
  try {
    const summaries = memoryRepository.getSummaries();
    return c.json({
      success: true,
      data: summaries.map(s => ({
        ...s,
        updatedAt: s.updatedAt.toISOString(),
      })),
    });
  } catch (error) {
    logger.error('Failed to get memory summaries', error);
    return c.json({ success: false, error: 'Failed to get summaries' }, 500);
  }
});

// GET /api/memory/:sessionId - Get memory for a specific session
app.get('/:sessionId', (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const memory = memoryRepository.findBySessionId(sessionId);

    if (!memory) {
      return c.json({ success: false, error: 'Memory not found' }, 404);
    }

    return c.json({
      success: true,
      data: {
        ...memory,
        createdAt: memory.createdAt.toISOString(),
        updatedAt: memory.updatedAt.toISOString(),
      },
    });
  } catch (error) {
    logger.error('Failed to get memory', error);
    return c.json({ success: false, error: 'Failed to get memory' }, 500);
  }
});

// GET /api/memory/:sessionId/context - Get recovery context for a session
app.get('/:sessionId/context', (c) => {
  const sessionId = c.req.param('sessionId');
  const minimal = c.req.query('minimal') === 'true';

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const memory = memoryRepository.findBySessionId(sessionId);

    if (!memory) {
      return c.json({ success: false, error: 'Memory not found' }, 404);
    }

    const context = minimal ? buildMinimalContext(memory) : buildContext(memory);

    return c.json({
      success: true,
      data: {
        sessionId: memory.sessionId,
        context,
        iterationCount: memory.iterationCount,
      },
    });
  } catch (error) {
    logger.error('Failed to get memory context', error);
    return c.json({ success: false, error: 'Failed to get context' }, 500);
  }
});

// POST /api/memory - Create or update memory
app.post('/', async (c) => {
  let body: unknown;
  try {
    body = await c.req.json();
  } catch {
    return c.json({ success: false, error: 'Invalid JSON body' }, 400);
  }

  // Validate body
  if (!body || typeof body !== 'object') {
    return c.json({ success: false, error: 'Request body must be an object' }, 400);
  }

  const data = body as Record<string, unknown>;

  if (!data.sessionId || typeof data.sessionId !== 'string') {
    return c.json({ success: false, error: 'sessionId is required' }, 400);
  }

  if (!isValidSessionId(data.sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const upsertData: MemoryUpsertData = {
      sessionId: data.sessionId,
      directory: typeof data.directory === 'string' ? data.directory : undefined,
      lastProgress: typeof data.lastProgress === 'string' ? data.lastProgress : undefined,
      pendingTasks: Array.isArray(data.pendingTasks) ? data.pendingTasks : undefined,
      completedTasks: Array.isArray(data.completedTasks) ? data.completedTasks : undefined,
      knownIssues: Array.isArray(data.knownIssues) ? data.knownIssues : undefined,
      decisions: Array.isArray(data.decisions) ? data.decisions : undefined,
      notes: typeof data.notes === 'string' ? data.notes : undefined,
      handoffNotes: typeof data.handoffNotes === 'string' ? data.handoffNotes : undefined,
      handoffPriority: Array.isArray(data.handoffPriority) ? data.handoffPriority : undefined,
      iterationCount: typeof data.iterationCount === 'number' ? data.iterationCount : undefined,
      totalTokensUsed: typeof data.totalTokensUsed === 'number' ? data.totalTokensUsed : undefined,
    };

    const memory = memoryRepository.upsert(upsertData);

    return c.json({
      success: true,
      data: {
        ...memory,
        createdAt: memory.createdAt.toISOString(),
        updatedAt: memory.updatedAt.toISOString(),
      },
    });
  } catch (error) {
    logger.error('Failed to upsert memory', error);
    return c.json({ success: false, error: 'Failed to save memory' }, 500);
  }
});

// PATCH /api/memory/:sessionId - Partial update memory
app.patch('/:sessionId', async (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  // Check if memory exists
  const existing = memoryRepository.findBySessionId(sessionId);
  if (!existing) {
    return c.json({ success: false, error: 'Memory not found' }, 404);
  }

  let body: unknown;
  try {
    body = await c.req.json();
  } catch {
    return c.json({ success: false, error: 'Invalid JSON body' }, 400);
  }

  if (!body || typeof body !== 'object') {
    return c.json({ success: false, error: 'Request body must be an object' }, 400);
  }

  const data = body as Record<string, unknown>;

  try {
    const upsertData: MemoryUpsertData = {
      sessionId,
      directory: typeof data.directory === 'string' ? data.directory : undefined,
      lastProgress: typeof data.lastProgress === 'string' ? data.lastProgress : undefined,
      pendingTasks: Array.isArray(data.pendingTasks) ? data.pendingTasks : undefined,
      completedTasks: Array.isArray(data.completedTasks) ? data.completedTasks : undefined,
      knownIssues: Array.isArray(data.knownIssues) ? data.knownIssues : undefined,
      decisions: Array.isArray(data.decisions) ? data.decisions : undefined,
      notes: typeof data.notes === 'string' ? data.notes : undefined,
      handoffNotes: typeof data.handoffNotes === 'string' ? data.handoffNotes : undefined,
      handoffPriority: Array.isArray(data.handoffPriority) ? data.handoffPriority : undefined,
      iterationCount: typeof data.iterationCount === 'number' ? data.iterationCount : undefined,
      totalTokensUsed: typeof data.totalTokensUsed === 'number' ? data.totalTokensUsed : undefined,
    };

    const memory = memoryRepository.upsert(upsertData);

    return c.json({
      success: true,
      data: {
        ...memory,
        createdAt: memory.createdAt.toISOString(),
        updatedAt: memory.updatedAt.toISOString(),
      },
    });
  } catch (error) {
    logger.error('Failed to update memory', error);
    return c.json({ success: false, error: 'Failed to update memory' }, 500);
  }
});

// DELETE /api/memory/:sessionId - Delete memory
app.delete('/:sessionId', (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const deleted = memoryRepository.delete(sessionId);

    if (!deleted) {
      return c.json({ success: false, error: 'Memory not found' }, 404);
    }

    return c.json({ success: true });
  } catch (error) {
    logger.error('Failed to delete memory', error);
    return c.json({ success: false, error: 'Failed to delete memory' }, 500);
  }
});

// POST /api/memory/:sessionId/increment - Increment iteration count
app.post('/:sessionId/increment', async (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  let tokensUsed = 0;
  try {
    const body = await c.req.json();
    if (body && typeof body.tokensUsed === 'number') {
      tokensUsed = body.tokensUsed;
    }
  } catch {
    // No body is OK
  }

  try {
    const existing = memoryRepository.findBySessionId(sessionId);
    if (!existing) {
      return c.json({ success: false, error: 'Memory not found' }, 404);
    }

    memoryRepository.incrementIteration(sessionId, tokensUsed);

    const updated = memoryRepository.findBySessionId(sessionId);
    return c.json({
      success: true,
      data: {
        iterationCount: updated?.iterationCount,
        totalTokensUsed: updated?.totalTokensUsed,
      },
    });
  } catch (error) {
    logger.error('Failed to increment iteration', error);
    return c.json({ success: false, error: 'Failed to increment' }, 500);
  }
});

// POST /api/memory/:sessionId/clear-handoff - Clear handoff data
app.post('/:sessionId/clear-handoff', (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const existing = memoryRepository.findBySessionId(sessionId);
    if (!existing) {
      return c.json({ success: false, error: 'Memory not found' }, 404);
    }

    memoryRepository.clearHandoff(sessionId);

    return c.json({ success: true });
  } catch (error) {
    logger.error('Failed to clear handoff', error);
    return c.json({ success: false, error: 'Failed to clear handoff' }, 500);
  }
});

export default app;
