/**
 * Session domain exports
 */

// Entity and types
export type {
  Session,
  SessionListItem,
  ParsedSessionData,
  AggregatedSession,
  AggregatedSessionListItem,
  CreateSessionInput,
  UpdateSessionInput,
  SessionStats,
} from './entity.js';

export { generateTitle } from './entity.js';

// Value objects
export type {
  SessionStatus,
  ToolCallInfo,
  ProcessInfo,
  ClaudeSessionFile,
} from './value-objects.js';

export {
  SESSION_STATUSES,
  isActiveStatus,
  needsAttention,
} from './value-objects.js';

// Events
export type {
  SessionEvent,
  SessionDiscovered,
  SessionUpdated,
  SessionLost,
  SessionRecovered,
  SessionCompleted,
  ToolUsageEvent,
  ScanComplete,
  SessionDomainEvent,
} from './events.js';

export {
  createSessionDiscoveredEvent,
  createSessionUpdatedEvent,
  createSessionLostEvent,
  createSessionRecoveredEvent,
  createSessionCompletedEvent,
  createScanCompleteEvent,
} from './events.js';

// Repository interface
export type {
  ActiveSessionRecord,
  ISessionRepository,
  SessionUpsertData,
} from './repository.js';
