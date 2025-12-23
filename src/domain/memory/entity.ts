/**
 * Memory domain entities
 *
 * Implements the "relay race" pattern from Continuous Claude:
 * Each iteration does one thing, leaves notes for the next.
 */

import type { Entity } from '../shared/types.js';

/** Session memory - persisted context for recovery */
export interface SessionMemory extends Entity {
  /** Related session ID */
  sessionId: string;

  /** Working directory */
  directory: string;

  /** Progress tracking */
  lastProgress: string;
  pendingTasks: string[];
  completedTasks: string[];

  /** Context information */
  knownIssues: string[];
  decisions: string[];
  notes: string;

  /** Handoff information (for next iteration) */
  handoffNotes: string;
  handoffPriority: string[];

  /** Metadata */
  iterationCount: number;
  totalTokensUsed: number;
}

/** Input for creating/updating memory */
export interface MemoryUpsertData {
  sessionId: string;
  directory?: string;
  lastProgress?: string;
  pendingTasks?: string[];
  completedTasks?: string[];
  knownIssues?: string[];
  decisions?: string[];
  notes?: string;
  handoffNotes?: string;
  handoffPriority?: string[];
  iterationCount?: number;
  totalTokensUsed?: number;
}

/** Memory summary for quick display */
export interface MemorySummary {
  sessionId: string;
  directory: string;
  lastProgress: string;
  pendingTaskCount: number;
  completedTaskCount: number;
  iterationCount: number;
  updatedAt: Date;
}

/** Create an empty memory for a session */
export function createEmptyMemory(sessionId: string, directory: string): Omit<SessionMemory, 'id' | 'createdAt' | 'updatedAt'> {
  return {
    sessionId,
    directory,
    lastProgress: '',
    pendingTasks: [],
    completedTasks: [],
    knownIssues: [],
    decisions: [],
    notes: '',
    handoffNotes: '',
    handoffPriority: [],
    iterationCount: 0,
    totalTokensUsed: 0,
  };
}

/** Convert memory to summary */
export function toMemorySummary(memory: SessionMemory): MemorySummary {
  return {
    sessionId: memory.sessionId,
    directory: memory.directory,
    lastProgress: memory.lastProgress,
    pendingTaskCount: memory.pendingTasks.length,
    completedTaskCount: memory.completedTasks.length,
    iterationCount: memory.iterationCount,
    updatedAt: memory.updatedAt,
  };
}
