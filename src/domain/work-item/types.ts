export const WORK_ITEM_STATUSES = [
  'inbox',
  'planned',
  'active',
  'blocked',
  'done',
  'archived',
] as const;

export const WORK_ITEM_KINDS = [
  'todo',
  'idea',
  'note',
  'project_task',
] as const;

export const WORK_ITEM_STATUS_SOURCES = [
  'user',
  'accepted_agent_suggestion',
] as const;

export type WorkItemStatus = typeof WORK_ITEM_STATUSES[number];
export type WorkItemKind = typeof WORK_ITEM_KINDS[number];
export type WorkItemStatusSource = typeof WORK_ITEM_STATUS_SOURCES[number];

export interface Area {
  id: string;
  name: string;
  projectRoot?: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface WorkItem {
  id: string;
  kind: WorkItemKind;
  status: WorkItemStatus;
  title: string;
  body?: string;
  projectRoot?: string;
  areaId?: string;
  area?: string;
  statusSource: WorkItemStatusSource;
  createdAt: Date;
  updatedAt: Date;
  completedAt?: Date;
}

export interface WorkItemCreateInput {
  kind?: WorkItemKind;
  status?: WorkItemStatus;
  title: string;
  body?: string;
  projectRoot?: string;
  area?: string;
  statusSource?: WorkItemStatusSource;
}

export interface WorkItemUpdateInput {
  kind?: WorkItemKind;
  status?: WorkItemStatus;
  title?: string;
  body?: string | null;
  projectRoot?: string | null;
  area?: string | null;
  statusSource?: WorkItemStatusSource;
}

export interface WorkItemFilters {
  status?: WorkItemStatus[];
  kind?: WorkItemKind;
  projectRoot?: string;
  includeArchived?: boolean;
}

export interface WorkItemOverviewStats {
  total: number;
  inbox: number;
  planned: number;
  active: number;
  blocked: number;
  done: number;
  archived: number;
  todo: number;
  idea: number;
  note: number;
  projectTask: number;
}

export function isWorkItemStatus(value: unknown): value is WorkItemStatus {
  return typeof value === 'string' && WORK_ITEM_STATUSES.includes(value as WorkItemStatus);
}

export function isWorkItemKind(value: unknown): value is WorkItemKind {
  return typeof value === 'string' && WORK_ITEM_KINDS.includes(value as WorkItemKind);
}

export function isWorkItemStatusSource(value: unknown): value is WorkItemStatusSource {
  return typeof value === 'string' &&
    WORK_ITEM_STATUS_SOURCES.includes(value as WorkItemStatusSource);
}
