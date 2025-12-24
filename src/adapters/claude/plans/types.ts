/**
 * Plan types for Claude Code plan files
 */

/** A single task item from the plan */
export interface PlanTask {
  text: string;
  completed: boolean;
  phase?: string;
}

/** A phase in the plan */
export interface PlanPhase {
  name: string;
  title: string;
  tasks: PlanTask[];
  completedCount: number;
  totalCount: number;
}

/** Parsed plan data */
export interface Plan {
  /** Unique identifier (filename without extension) */
  id: string;
  /** Full file path */
  filePath: string;
  /** Plan title (first H1 heading) */
  title: string;
  /** File modification time */
  modifiedAt: Date;
  /** File creation time */
  createdAt: Date;
  /** Raw markdown content */
  content: string;
  /** Extracted phases */
  phases: PlanPhase[];
  /** All tasks (flattened) */
  tasks: PlanTask[];
  /** Task statistics */
  stats: {
    totalTasks: number;
    completedTasks: number;
    pendingTasks: number;
    completionPercent: number;
    phaseCount: number;
  };
}

/** Summary for quick display */
export interface PlanSummary {
  id: string;
  title: string;
  modifiedAt: Date;
  completedTasks: number;
  totalTasks: number;
  completionPercent: number;
  phaseCount: number;
}
