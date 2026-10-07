import type { WorkItem } from "../../types/work-item";
import type { LedgerDetail } from "../../../../../domain/ledger/types";
export type LedgerView =
  | "overview"
  | "todos"
  | "goals"
  | "review"
  | "ledger-settings";
export interface Todo extends WorkItem {
  readyToComplete: boolean;
  checklist: Array<{ id: string; text: string; evidenced: boolean; satisfied: boolean }>;
  sessions: Array<{
    runtime_session_id: string;
    title: string;
    status: string;
    needsInput?: boolean;
    statusReason?: string | null;
    last_active_at?: string;
  }>;
}
export interface Goal extends WorkItem {
  todos: Todo[];
  progress: { done: number; total: number; active: number };
  weeklyMovement: number;
  stale: boolean;
  recent: Array<{ id: string; kind: string; title: string; at: string; todoId: string; sessionId: string | null }>;
}
export interface Review {
  open: LedgerDetail[];
  accepted: LedgerDetail[];
  offPlan: Array<{ id: string; title: string; sessionId: string }>;
  corrections: Array<{ title: string; created_at: string }>;
  unattributedRuntimeShare: number;
  goals: Goal[];
}
export interface LedgerProps {
  token: string;
  view: LedgerView;
  onOpenSession: (id: string) => void;
}
