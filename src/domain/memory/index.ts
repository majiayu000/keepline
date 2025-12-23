/**
 * Memory domain exports
 *
 * Provides session memory persistence for the "relay race" pattern.
 */

// Entities and types
export type {
  SessionMemory,
  MemoryUpsertData,
  MemorySummary,
} from './entity.js';

export { createEmptyMemory, toMemorySummary } from './entity.js';

// Context builder
export type { ContextBuilderOptions } from './context-builder.js';
export { buildContext, buildMinimalContext, hasContent } from './context-builder.js';

// Extractor
export type { HookEventData } from './extractor.js';
export {
  extractFromOutput,
  extractFromHookEvent,
  mergeExtracted,
} from './extractor.js';
