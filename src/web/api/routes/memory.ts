/**
 * Memory Routes
 *
 * Handles session memory operations for the "relay race" pattern
 * and semantic search with vector embeddings.
 */

import { Hono } from 'hono';
import { memoryRepository } from '../../../infrastructure/database/index.js';
import { buildContext, buildMinimalContext } from '../../../domain/memory/index.js';
import type { MemoryUpsertData } from '../../../domain/memory/index.js';
import { logger } from '../../../lib/logger.js';
import { authMiddleware } from '../middleware/auth.js';
import { isValidSessionId } from '../middleware/validation.js';
import type { ObservationCategory } from '../../../infrastructure/vector/types.js';

const app = new Hono();
app.use('*', authMiddleware);

async function getVectorServices() {
  const [{ getVectorStore }, { getEmbeddingService }] = await Promise.all([
    import('../../../infrastructure/vector/lancedb.adapter.js'),
    import('../../../infrastructure/vector/embedding.service.js'),
  ]);

  return {
    vectorStore: getVectorStore(),
    embeddingService: getEmbeddingService(),
  };
}

async function getVectorStoreService() {
  const { getVectorStore } = await import('../../../infrastructure/vector/lancedb.adapter.js');
  return getVectorStore();
}

async function getEmbeddingServiceInstance() {
  const { getEmbeddingService } = await import('../../../infrastructure/vector/embedding.service.js');
  return getEmbeddingService();
}

async function getEndlessModeServiceInstance() {
  const { getEndlessModeService } = await import('../../../services/endless.mode.js');
  return getEndlessModeService();
}

async function getCompressionQueueInstance() {
  const { getCompressionQueue } = await import('../../../services/compression.queue.js');
  return getCompressionQueue();
}

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

// ============================================
// Semantic Search API (Vector Store)
// ============================================

// GET /api/memory/search - Semantic search for observations
app.get('/search', async (c) => {
  const query = c.req.query('query');
  const limit = c.req.query('limit');
  const sessionId = c.req.query('sessionId');
  const category = c.req.query('category') as ObservationCategory | undefined;
  const minScore = c.req.query('minScore');

  if (!query || query.trim().length === 0) {
    return c.json({ success: false, error: 'Query parameter is required' }, 400);
  }

  try {
    const { vectorStore, embeddingService } = await getVectorServices();

    // Initialize stores if needed
    await vectorStore.initialize();

    // Generate embedding for query
    const queryVector = await embeddingService.embed(query);

    // Search for similar observations
    const results = await vectorStore.search(queryVector, {
      limit: limit ? parseInt(limit, 10) : 10,
      sessionId: sessionId && isValidSessionId(sessionId) ? sessionId : undefined,
      category,
      minScore: minScore ? parseFloat(minScore) : undefined,
    });

    return c.json({
      success: true,
      data: {
        query,
        results: results.map((r) => ({
          ...r.observation,
          timestamp: r.observation.timestamp.toISOString(),
          score: r.score,
          distance: r.distance,
        })),
        count: results.length,
        provider: embeddingService.getProvider(),
      },
    });
  } catch (error) {
    logger.error('Semantic search failed', error);
    return c.json({ success: false, error: 'Search failed' }, 500);
  }
});

// GET /api/memory/observations - List all observations
app.get('/observations', async (c) => {
  const sessionId = c.req.query('sessionId');

  try {
    const vectorStore = await getVectorStoreService();
    await vectorStore.initialize();

    if (sessionId && isValidSessionId(sessionId)) {
      const observations = await vectorStore.getBySessionId(sessionId);
      return c.json({
        success: true,
        data: observations.map((o) => ({
          ...o,
          timestamp: o.timestamp.toISOString(),
        })),
        count: observations.length,
      });
    }

    // Return total count if no sessionId
    const count = await vectorStore.count();
    return c.json({
      success: true,
      data: { totalObservations: count },
    });
  } catch (error) {
    logger.error('Failed to list observations', error);
    return c.json({ success: false, error: 'Failed to list observations' }, 500);
  }
});

// GET /api/memory/observations/:id - Get observation by ID
app.get('/observations/:id', async (c) => {
  const id = c.req.param('id');

  try {
    const vectorStore = await getVectorStoreService();
    await vectorStore.initialize();

    const observation = await vectorStore.getById(id);

    if (!observation) {
      return c.json({ success: false, error: 'Observation not found' }, 404);
    }

    return c.json({
      success: true,
      data: {
        ...observation,
        timestamp: observation.timestamp.toISOString(),
      },
    });
  } catch (error) {
    logger.error('Failed to get observation', error);
    return c.json({ success: false, error: 'Failed to get observation' }, 500);
  }
});

// POST /api/memory/observations - Add a new observation
app.post('/observations', async (c) => {
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

  // Validate required fields
  if (!data.sessionId || typeof data.sessionId !== 'string') {
    return c.json({ success: false, error: 'sessionId is required' }, 400);
  }
  if (!data.content || typeof data.content !== 'string') {
    return c.json({ success: false, error: 'content is required' }, 400);
  }
  if (!data.category || typeof data.category !== 'string') {
    return c.json({ success: false, error: 'category is required' }, 400);
  }

  const validCategories: ObservationCategory[] = [
    'decision',
    'bugfix',
    'feature',
    'refactor',
    'discovery',
    'change',
  ];
  if (!validCategories.includes(data.category as ObservationCategory)) {
    return c.json({
      success: false,
      error: `Invalid category. Must be one of: ${validCategories.join(', ')}`,
    }, 400);
  }

  try {
    const { vectorStore, embeddingService } = await getVectorServices();

    await vectorStore.initialize();

    // Generate embedding for content
    const vector = await embeddingService.embed(data.content);

    // Create observation
    const observation = {
      id: crypto.randomUUID(),
      sessionId: data.sessionId,
      content: data.content,
      category: data.category as ObservationCategory,
      files: Array.isArray(data.files) ? data.files : [],
      concepts: Array.isArray(data.concepts) ? data.concepts : [],
      timestamp: new Date(),
      tokenCount: typeof data.tokenCount === 'number' ? data.tokenCount : Math.ceil(data.content.length / 4),
      compressed: typeof data.compressed === 'boolean' ? data.compressed : false,
    };

    await vectorStore.insert(observation, vector);

    return c.json({
      success: true,
      data: {
        ...observation,
        timestamp: observation.timestamp.toISOString(),
      },
    });
  } catch (error) {
    logger.error('Failed to create observation', error);
    return c.json({ success: false, error: 'Failed to create observation' }, 500);
  }
});

// DELETE /api/memory/observations/:id - Delete observation by ID
app.delete('/observations/:id', async (c) => {
  const id = c.req.param('id');

  try {
    const vectorStore = await getVectorStoreService();
    await vectorStore.initialize();

    const deleted = await vectorStore.delete(id);

    if (!deleted) {
      return c.json({ success: false, error: 'Observation not found' }, 404);
    }

    return c.json({ success: true });
  } catch (error) {
    logger.error('Failed to delete observation', error);
    return c.json({ success: false, error: 'Failed to delete observation' }, 500);
  }
});

// DELETE /api/memory/observations/session/:sessionId - Delete all observations for a session
app.delete('/observations/session/:sessionId', async (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const vectorStore = await getVectorStoreService();
    await vectorStore.initialize();

    const count = await vectorStore.deleteBySessionId(sessionId);

    return c.json({
      success: true,
      data: { deletedCount: count },
    });
  } catch (error) {
    logger.error('Failed to delete observations', error);
    return c.json({ success: false, error: 'Failed to delete observations' }, 500);
  }
});

// GET /api/memory/embedding/stats - Get embedding service stats
app.get('/embedding/stats', async (c) => {
  try {
    const embeddingService = await getEmbeddingServiceInstance();
    const stats = embeddingService.getCacheStats();

    return c.json({
      success: true,
      data: stats,
    });
  } catch (error) {
    logger.error('Failed to get embedding stats', error);
    return c.json({ success: false, error: 'Failed to get stats' }, 500);
  }
});

// ============================================
// Endless Mode API
// ============================================

// GET /api/memory/endless/stats - Get endless mode statistics
app.get('/endless/stats', async (c) => {
  try {
    const endlessMode = await getEndlessModeServiceInstance();
    const stats = endlessMode.getStats();

    return c.json({
      success: true,
      data: {
        ...stats,
        lastCompressionAt: stats.lastCompressionAt?.toISOString() ?? null,
      },
    });
  } catch (error) {
    logger.error('Failed to get endless mode stats', error);
    return c.json({ success: false, error: 'Failed to get stats' }, 500);
  }
});

// GET /api/memory/endless/working/:sessionId - Get working memory for a session
app.get('/endless/working/:sessionId', async (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const endlessMode = await getEndlessModeServiceInstance();
    const items = endlessMode.getWorkingMemory(sessionId);
    const tokenCount = endlessMode.getSessionTokenCount(sessionId);

    return c.json({
      success: true,
      data: {
        sessionId,
        items: items.map((item) => ({
          ...item,
          timestamp: item.timestamp.toISOString(),
        })),
        itemCount: items.length,
        tokenCount,
      },
    });
  } catch (error) {
    logger.error('Failed to get working memory', error);
    return c.json({ success: false, error: 'Failed to get working memory' }, 500);
  }
});

// GET /api/memory/endless/context/:sessionId - Get working memory as formatted context
app.get('/endless/context/:sessionId', async (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const endlessMode = await getEndlessModeServiceInstance();
    const context = endlessMode.getWorkingMemoryContext(sessionId);
    const tokenCount = endlessMode.getSessionTokenCount(sessionId);

    return c.json({
      success: true,
      data: {
        sessionId,
        context,
        tokenCount,
      },
    });
  } catch (error) {
    logger.error('Failed to get working memory context', error);
    return c.json({ success: false, error: 'Failed to get context' }, 500);
  }
});

// POST /api/memory/endless/compress/:sessionId - Trigger compression for a session
app.post('/endless/compress/:sessionId', async (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const endlessMode = await getEndlessModeServiceInstance();
    const statsBefore = endlessMode.getStats();

    await endlessMode.compressWorkingMemory(sessionId);

    const statsAfter = endlessMode.getStats();

    return c.json({
      success: true,
      data: {
        sessionId,
        itemsCompressed: statsAfter.archivedItems - statsBefore.archivedItems,
        remainingItems: endlessMode.getWorkingMemory(sessionId).length,
        remainingTokens: endlessMode.getSessionTokenCount(sessionId),
      },
    });
  } catch (error) {
    logger.error('Failed to compress working memory', error);
    return c.json({ success: false, error: 'Compression failed' }, 500);
  }
});

// POST /api/memory/endless/archive/:sessionId - Archive all session memory
app.post('/endless/archive/:sessionId', async (c) => {
  const sessionId = c.req.param('sessionId');

  if (!isValidSessionId(sessionId)) {
    return c.json({ success: false, error: 'Invalid session ID format' }, 400);
  }

  try {
    const endlessMode = await getEndlessModeServiceInstance();
    const itemCount = endlessMode.getWorkingMemory(sessionId).length;

    await endlessMode.archiveSession(sessionId);

    return c.json({
      success: true,
      data: {
        sessionId,
        itemsArchived: itemCount,
      },
    });
  } catch (error) {
    logger.error('Failed to archive session', error);
    return c.json({ success: false, error: 'Archive failed' }, 500);
  }
});

// GET /api/memory/compression/stats - Get compression queue statistics
app.get('/compression/stats', async (c) => {
  try {
    const queue = await getCompressionQueueInstance();
    const stats = queue.getStats();

    return c.json({
      success: true,
      data: stats,
    });
  } catch (error) {
    logger.error('Failed to get compression stats', error);
    return c.json({ success: false, error: 'Failed to get stats' }, 500);
  }
});

export default app;
