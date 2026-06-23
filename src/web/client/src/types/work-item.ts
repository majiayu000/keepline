export type WorkItemStatus = 'inbox' | 'planned' | 'active' | 'blocked' | 'done' | 'archived'
export type WorkItemKind = 'todo' | 'idea' | 'note' | 'project_task'
export type WorkItemStatusSource = 'user' | 'accepted_agent_suggestion'

export interface WorkItem {
  id: string
  kind: WorkItemKind
  status: WorkItemStatus
  title: string
  body?: string
  projectRoot?: string
  areaId?: string
  area?: string
  statusSource: WorkItemStatusSource
  createdAt: string
  updatedAt: string
  completedAt?: string
}

export interface WorkItemOverviewStats {
  total: number
  inbox: number
  planned: number
  active: number
  blocked: number
  done: number
  archived: number
  todo: number
  idea: number
  note: number
  projectTask: number
}

export interface WorkItemsData {
  items: WorkItem[]
  stats: WorkItemOverviewStats
}

export interface WorkItemCreateInput {
  title: string
  kind?: WorkItemKind
  status?: WorkItemStatus
  body?: string
  projectRoot?: string
  area?: string
  statusSource?: WorkItemStatusSource
}

export interface WorkItemUpdateInput {
  title?: string
  kind?: WorkItemKind
  status?: WorkItemStatus
  body?: string | null
  projectRoot?: string | null
  area?: string | null
  statusSource?: WorkItemStatusSource
}
