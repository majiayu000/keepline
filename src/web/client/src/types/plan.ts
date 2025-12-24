/**
 * Plan types for Claude Code plan files
 */

/** A single task item from the plan */
export interface PlanTask {
  text: string
  completed: boolean
  phase?: string
}

/** A phase in the plan */
export interface PlanPhase {
  name: string
  title: string
  completedCount: number
  totalCount: number
  tasks?: PlanTask[]
}

/** Plan statistics */
export interface PlanStats {
  totalTasks: number
  completedTasks: number
  pendingTasks: number
  completionPercent: number
  phaseCount: number
}

/** Plan data from API */
export interface Plan {
  id: string
  title: string
  filePath?: string
  modifiedAt: string
  createdAt: string
  content?: string
  phases: PlanPhase[]
  tasks?: PlanTask[]
  stats: PlanStats
}

/** Plan summary for quick display */
export interface PlanSummary {
  id: string
  title: string
  modifiedAt: string
  completedTasks: number
  totalTasks: number
  completionPercent: number
  phaseCount: number
}

/** Aggregate plan stats */
export interface PlanAggregateStats {
  totalPlans: number
  totalTasks: number
  completedTasks: number
  overallCompletion: number
}
