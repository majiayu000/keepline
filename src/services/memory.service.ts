/**
 * Memory Service
 *
 * Connects hook events to the memory system.
 * Implements the "relay race" pattern by automatically
 * extracting and persisting progress from tool events.
 */

import { on } from '../lib/events.js';
import { logger } from '../lib/logger.js';
import { memoryRepository } from '../infrastructure/database/repositories/memory.repository.js';
import {
  extractFromHookEvent,
  mergeExtracted,
  type HookEventData,
} from '../domain/memory/extractor.js';
import type { ToolEventPayload } from '../lib/events.js';
import type { MemoryUpsertData } from '../domain/memory/entity.js';

/** Track active session directories */
const sessionDirectories = new Map<string, string>();

/** Initialize memory service - subscribe to hook events */
export function initializeMemoryService(): void {
  logger.info('Initializing memory service...');

  // Subscribe to tool events
  on('tool:post', handleToolEvent);

  // Subscribe to session events to track directories
  on('session:discovered', (payload) => {
    if (payload.session.directory) {
      sessionDirectories.set(payload.session.id, payload.session.directory);
    }
  });

  on('session:updated', (payload) => {
    if (payload.session.directory) {
      sessionDirectories.set(payload.session.id, payload.session.directory);
    }
  });

  logger.info('Memory service initialized - listening for hook events');
}

/**
 * Handle tool event and update memory
 */
async function handleToolEvent(payload: ToolEventPayload): Promise<void> {
  try {
    const { sessionId, tool, input, timestamp } = payload;

    // Convert to HookEventData format
    const hookEvent: HookEventData = {
      sessionId,
      toolName: tool,
      toolInput: input,
      timestamp,
    };

    // Extract progress from the event
    const extracted = extractFromHookEvent(hookEvent);

    if (!extracted.lastProgress) {
      // No meaningful progress to extract
      return;
    }

    // Get existing memory or create new
    const existing = memoryRepository.findBySessionId(sessionId);
    const directory = sessionDirectories.get(sessionId) || '';

    if (existing) {
      // Merge with existing data
      const merged = mergeExtracted(
        {
          sessionId: existing.sessionId,
          directory: existing.directory,
          lastProgress: existing.lastProgress,
          pendingTasks: existing.pendingTasks,
          completedTasks: existing.completedTasks,
          knownIssues: existing.knownIssues,
          decisions: existing.decisions,
        },
        extracted
      );

      memoryRepository.upsert(merged as MemoryUpsertData);
      logger.debug(`Memory updated for session ${sessionId}: ${extracted.lastProgress}`);
    } else {
      // Create new memory entry
      const newMemory: MemoryUpsertData = {
        sessionId,
        directory,
        lastProgress: extracted.lastProgress || '',
        pendingTasks: [],
        completedTasks: [],
        knownIssues: [],
        decisions: [],
      };

      memoryRepository.upsert(newMemory);
      logger.debug(`Memory created for session ${sessionId}: ${extracted.lastProgress}`);
    }
  } catch (error) {
    logger.error('Failed to handle tool event for memory', error);
  }
}

/**
 * Manually update memory from Claude output text
 */
export function updateMemoryFromOutput(
  sessionId: string,
  output: string,
  directory?: string
): void {
  const { extractFromOutput, mergeExtracted } = require('../domain/memory/extractor.js');

  try {
    const extracted = extractFromOutput(output);

    if (
      !extracted.completedTasks?.length &&
      !extracted.pendingTasks?.length &&
      !extracted.knownIssues?.length &&
      !extracted.decisions?.length
    ) {
      // No meaningful progress extracted
      return;
    }

    const existing = memoryRepository.findBySessionId(sessionId);

    if (existing) {
      const merged = mergeExtracted(
        {
          sessionId: existing.sessionId,
          directory: existing.directory,
          lastProgress: existing.lastProgress,
          pendingTasks: existing.pendingTasks,
          completedTasks: existing.completedTasks,
          knownIssues: existing.knownIssues,
          decisions: existing.decisions,
        },
        extracted
      );

      memoryRepository.upsert(merged as MemoryUpsertData);
    } else if (directory) {
      const newMemory: MemoryUpsertData = {
        sessionId,
        directory,
        lastProgress: '',
        pendingTasks: extracted.pendingTasks || [],
        completedTasks: extracted.completedTasks || [],
        knownIssues: extracted.knownIssues || [],
        decisions: extracted.decisions || [],
      };

      memoryRepository.upsert(newMemory);
    }

    logger.debug(`Memory updated from output for session ${sessionId}`);
  } catch (error) {
    logger.error('Failed to update memory from output', error);
  }
}

/**
 * Get memory for a session
 */
export function getSessionMemory(sessionId: string) {
  return memoryRepository.findBySessionId(sessionId);
}

/**
 * Get all memories
 */
export function getAllMemories() {
  return memoryRepository.findAll();
}

/**
 * Get memory summaries
 */
export function getMemorySummaries() {
  return memoryRepository.getSummaries();
}

/**
 * Delete memory for a session
 */
export function deleteSessionMemory(sessionId: string): boolean {
  return memoryRepository.delete(sessionId);
}

/**
 * Update memory manually (for CLI/API)
 */
export function updateMemory(data: MemoryUpsertData) {
  return memoryRepository.upsert(data);
}
