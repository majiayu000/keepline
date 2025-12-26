/**
 * HTTP server for receiving hook events
 *
 * Integrates with the compression queue for async memory processing.
 */

import Fastify, { FastifyInstance } from 'fastify';
import { logger } from '../../lib/logger.js';
import { config } from '../../lib/config.js';
import { emit } from '../../lib/events.js';
import { updateSession } from '../../services/session.service.js';
import {
  getCompressionQueue,
  startCompressionQueue,
  stopCompressionQueue,
} from '../../services/compression.queue.js';
import { generateSessionContext } from '../../services/context.injection.js';
import type {
  HookEvent,
  ToolUseHookEvent,
  UserPromptSubmitHookEvent,
  HookEventType,
} from './types.js';

let server: FastifyInstance | null = null;

/** Valid hook event types */
const VALID_EVENT_TYPES: Set<HookEventType> = new Set([
  'PreToolUse',
  'PostToolUse',
  'Notification',
  'Stop',
  'UserPromptSubmit',
]);

/** Track first prompts per session for context injection */
const sessionFirstPrompts: Map<string, boolean> = new Map();

/** Validate hook event payload */
function isValidHookEvent(event: unknown): event is HookEvent {
  if (!event || typeof event !== 'object') {
    return false;
  }

  const e = event as Record<string, unknown>;

  // Required fields
  if (typeof e.session_id !== 'string' || e.session_id.length === 0) {
    return false;
  }
  if (typeof e.cwd !== 'string') {
    return false;
  }
  if (typeof e.timestamp !== 'string') {
    return false;
  }
  if (typeof e.event_type !== 'string' || !VALID_EVENT_TYPES.has(e.event_type as HookEventType)) {
    return false;
  }

  // Tool events require additional fields
  if (e.event_type === 'PreToolUse' || e.event_type === 'PostToolUse') {
    if (typeof e.tool_name !== 'string' || e.tool_name.length === 0) {
      return false;
    }
    if (!e.tool_input || typeof e.tool_input !== 'object') {
      return false;
    }
  }

  // UserPromptSubmit requires prompt field
  if (e.event_type === 'UserPromptSubmit') {
    if (typeof e.prompt !== 'string') {
      return false;
    }
  }

  return true;
}

/** Handle incoming hook event */
async function handleHookEvent(event: HookEvent): Promise<void> {
  logger.debug('Hook event received', { type: event.event_type, session: event.session_id });

  switch (event.event_type) {
    case 'PreToolUse':
    case 'PostToolUse': {
      const toolEvent = event as ToolUseHookEvent;

      // Emit tool event
      emit(event.event_type === 'PreToolUse' ? 'tool:pre' : 'tool:post', {
        sessionId: toolEvent.session_id,
        tool: toolEvent.tool_name,
        input: toolEvent.tool_input,
        output: toolEvent.tool_output,
        timestamp: new Date(toolEvent.timestamp),
      });

      // Update session with tool info
      updateSession(toolEvent.session_id, {
        lastTool: toolEvent.tool_name,
        lastToolInput: JSON.stringify(toolEvent.tool_input),
        lastActiveAt: new Date(toolEvent.timestamp),
        status: 'running',
      });

      // Extract current file if applicable
      const fileKeys = ['file_path', 'path', 'filePath', 'notebook_path'];
      for (const key of fileKeys) {
        const value = toolEvent.tool_input[key];
        if (typeof value === 'string') {
          updateSession(toolEvent.session_id, { currentFile: value });
          break;
        }
      }

      // Enqueue PostToolUse events for async compression (if output exists)
      if (event.event_type === 'PostToolUse' && toolEvent.tool_output) {
        const queue = getCompressionQueue();
        if (queue.isActive()) {
          queue.enqueue({
            toolName: toolEvent.tool_name,
            toolInput: toolEvent.tool_input,
            toolOutput: toolEvent.tool_output,
            sessionId: toolEvent.session_id,
          });
        }
      }
      break;
    }

    case 'Notification':
      // Just log notifications for now
      logger.info(`Notification from ${event.session_id}: ${(event as { message: string }).message}`);
      break;

    case 'Stop':
      // Session stopped
      updateSession(event.session_id, {
        status: 'completed',
        completedAt: new Date(event.timestamp),
      });

      // Clean up session tracking
      sessionFirstPrompts.delete(event.session_id);

      // Emit session:end event
      emit('session:end', {
        sessionId: event.session_id,
        timestamp: new Date(event.timestamp),
        reason: (event as { reason?: string }).reason,
      });
      break;

    case 'UserPromptSubmit': {
      const promptEvent = event as UserPromptSubmitHookEvent;

      // Check if this is the first prompt for this session
      const isFirstPrompt = !sessionFirstPrompts.has(event.session_id);
      if (isFirstPrompt) {
        sessionFirstPrompts.set(event.session_id, true);

        // Generate and log context for first prompt (async, don't block)
        generateSessionContext(event.cwd, promptEvent.prompt)
          .then((context) => {
            if (context.observations.length > 0) {
              logger.info(
                `Context injection available for ${event.session_id}: ` +
                `${context.observations.length} observations, ${context.totalTokens} tokens`
              );
              // Note: Actual injection into CLAUDE.md would require file system access
              // For now, we log the context for debugging
              logger.debug('Generated context block:', context.contextBlock);
            }
          })
          .catch((error) => {
            logger.error('Failed to generate session context', error);
          });
      }

      // Emit prompt event
      emit('session:updated', {
        session: {
          id: event.session_id,
          directory: event.cwd,
          startedAt: new Date(event.timestamp),
          status: 'running',
        } as import('../../domain/session/entity.js').Session,
      });
      break;
    }
  }
}

/** Start hook server */
export async function startHookServer(): Promise<void> {
  if (server) {
    logger.warn('Hook server already running');
    return;
  }

  const port = config.get().hookPort;

  server = Fastify({ logger: false });

  // Health check endpoint
  server.get('/health', async () => {
    const queue = getCompressionQueue();
    return {
      status: 'ok',
      compression: {
        active: queue.isActive(),
        stats: queue.getStats(),
      },
    };
  });

  // Hook event endpoint
  server.post<{ Body: unknown }>('/hook', async (request, reply) => {
    try {
      // Validate input before processing
      if (!isValidHookEvent(request.body)) {
        logger.warn('Invalid hook event received', { body: request.body });
        reply.status(400);
        return { success: false, error: 'Invalid hook event payload' };
      }

      await handleHookEvent(request.body);
      return { success: true };
    } catch (error) {
      logger.error('Failed to handle hook event', error);
      reply.status(500);
      return { success: false, error: (error as Error).message };
    }
  });

  // Compression queue stats endpoint
  server.get('/compression/stats', async () => {
    const queue = getCompressionQueue();
    return queue.getStats();
  });

  // Context injection endpoint - retrieve relevant memories for a project
  server.get<{
    Querystring: { path?: string; prompt?: string };
  }>('/context', async (request, reply) => {
    const { path, prompt } = request.query;

    if (!path) {
      reply.status(400);
      return { success: false, error: 'Missing required "path" query parameter' };
    }

    try {
      const context = await generateSessionContext(path, prompt);
      return {
        success: true,
        data: {
          observationCount: context.observations.length,
          totalTokens: context.totalTokens,
          searchQuery: context.searchQuery,
          contextBlock: context.contextBlock,
          observations: context.observations.map((obs) => ({
            id: obs.id,
            content: obs.content,
            category: obs.category,
            files: obs.files,
            timestamp: obs.timestamp.toISOString(),
          })),
        },
      };
    } catch (error) {
      logger.error('Failed to generate context', error);
      reply.status(500);
      return { success: false, error: (error as Error).message };
    }
  });

  try {
    await server.listen({ port, host: '127.0.0.1' });

    // Start compression queue
    startCompressionQueue();

    logger.info(`Hook server listening on port ${port} with compression queue enabled`);
  } catch (error) {
    logger.error('Failed to start hook server', error);
    throw error;
  }
}

/** Stop hook server */
export async function stopHookServer(): Promise<void> {
  if (!server) return;

  const currentServer = server;
  try {
    // Stop compression queue first (flush pending items)
    await stopCompressionQueue();

    await currentServer.close();
    server = null;
    logger.info('Hook server stopped');
  } catch (error) {
    logger.error('Failed to stop hook server', error);
    // Only clear server reference if close actually succeeded
    // If it failed, server might still be running
    throw error;
  }
}

/** Check if hook server is running */
export function isHookServerRunning(): boolean {
  return server !== null;
}

/** Get hook server URL */
export function getHookServerUrl(): string {
  const port = config.get().hookPort;
  return `http://127.0.0.1:${port}`;
}
