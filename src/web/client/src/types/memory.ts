/**
 * Memory types for session context persistence
 * Implements the "relay race" pattern from Continuous Claude
 */

/** Session memory - persisted context for recovery */
export interface SessionMemory {
  id: string
  sessionId: string
  directory: string

  /** Progress tracking */
  lastProgress: string
  pendingTasks: string[]
  completedTasks: string[]

  /** Context information */
  knownIssues: string[]
  decisions: string[]
  notes: string

  /** Handoff information (for next iteration) */
  handoffNotes: string
  handoffPriority: string[]

  /** Metadata */
  iterationCount: number
  totalTokensUsed: number

  createdAt: string
  updatedAt: string
}

/** Memory summary for quick display */
export interface MemorySummary {
  sessionId: string
  directory: string
  lastProgress: string
  pendingTaskCount: number
  completedTaskCount: number
  iterationCount: number
  updatedAt: string
}

/** Memory context response */
export interface MemoryContext {
  sessionId: string
  context: string
  iterationCount: number
}
