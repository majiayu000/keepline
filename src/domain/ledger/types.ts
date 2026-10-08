export type TranscriptFact =
  | { kind: 'user_message'; text: string; at: string; turnId?: string }
  | { kind: 'agent_message'; text: string; at: string; turnId?: string; final: boolean }
  | { kind: 'turn'; phase: 'started' | 'completed' | 'aborted'; at: string; turnId: string; reason?: string }
  | { kind: 'tool'; callId: string; name: string; input: unknown; at: string; turnId?: string;
      mutating: boolean; startedOrder?: number; completedOrder?: number; exitCode?: number; outputHead?: string; facts?: ToolEvidence[] }
  | { kind: 'limit'; scope: 'usage' | 'budget'; at: string };
export interface ToolEvidence { kind: 'command' | 'test' | 'file' | 'commit' | 'pr' | 'verdict'; value: string; exitCode?: number }
export interface Anchors {
  paths: string[]; commands: string[]; keywords: string[];
  /** Persisted user criteria without a format need explicit command confirmation. */
  commandFormat?: 'legacy-unconfirmed' | 'literal-v2';
  /** Original patterns remain available for attribution, never completion. */
  legacyCommands?: string[];
}
export interface Constraint { kind: 'path_forbidden' | 'no_public_api_change' | 'preserve_text'; value?: string }
export interface RequirementItem {
  id: string; ordinal: number; title: string; anchors: Anchors; constraints: Constraint[];
  source: 'work_item' | 'model' | 'fallback' | 'user'; status: 'todo' | 'doing' | 'done' | 'unverified';
  statusSource: 'rule' | 'model' | 'user'; evidenceIds: string[]; checklistId?: string; dropped?: boolean;
}
export interface Ask { id: string; text: string; authoredText: string; kind: 'initial' | 'addition' | 'question'; at: string; turnId?: string }
export interface LedgerEvidence extends ToolEvidence { id: string; callId: string; at: string }
export interface LedgerStep { callId: string; name: string; summary: string; at: string; turnId: string;
  itemId?: string; evidenceIds: string[]; violations: string[]; acceptedOffPlan: boolean }
export interface Correction { callId: string; itemId?: string; acceptOffPlan?: boolean }
export interface LedgerRule { matcher: Pick<Anchors, 'paths' | 'commands'>; itemId: string }
export interface OffPlanRun { id: string; callIds: string[]; at: string }
export interface LedgerClaim { text: string; evidenceIds: string[]; turnId?: string }
export interface LedgerAcceptance { turnId: string; decision: 'accepted' | 'accepted_with_gaps' | 'follow_up'; droppedItemIds: string[]; reason?: string; at: string }
export interface LedgerDetail {
  sessionId: string; agentSessionId: string; title: string; projectRoot: string; runtimeId: string;
  state: 'running' | 'needs_input' | 'review' | 'accepted' | 'stopped' | 'ended'; statusReason?: string | null;
  parentSessionId?: string;
  activity?: { action?: string; at?: string; lastMessage?: string; evidence: LedgerEvidence[] };
  subagents?: Array<Pick<LedgerDetail,'sessionId' | 'title' | 'state' | 'lastActiveAt' | 'activity'>>;
  possiblyWaiting: boolean; available: boolean; unavailableReason?: string;
  asks: Ask[]; items: RequirementItem[]; evidence: LedgerEvidence[]; trail: LedgerStep[];
  readOnlyCount: number; turns: Array<{ id: string; at: string; phase: string; readOnlyCount: number; durationMs: number }>;
  offPlan: OffPlanRun[]; claims: LedgerClaim[]; acceptances: LedgerAcceptance[];
  progress: { done: number; total: number }; lastActiveAt: string; turnId?: string;
  workItemId?: string; attribution?: Array<{ workItemId: string; title: string; score: number; reasons: string[] }>;
  limited?: boolean;
  followUpSuggestions?: string[];
  importSuggestions?: Array<{ sessionId: string; title: string }>;
}
export interface LedgerConfig {
  enabled: boolean; retentionDays: number; stalledAfterSeconds: number; alertCoalesceSeconds: number;
  judge: { enabled: boolean; backend: 'cli-claude' | 'cli-codex' | 'local' | 'sdk'; model: string | null };
  deviation: 'off' | 'conservative' | 'sensitive'; constraints: boolean;
  alerts: Record<'needs_input' | 'off_plan' | 'claimed_unverified' | 'stalled' | 'limited', boolean>;
  nativeNotifications: boolean; focus: { minutes: number; until: string | null }; staleGoalDays: number;
  exclude: { projects: string[]; runtimes: string[] };
}
export const DEFAULT_LEDGER_CONFIG: LedgerConfig = {
  enabled: true, retentionDays: 30, stalledAfterSeconds: 900, alertCoalesceSeconds: 600,
  judge: { enabled: false, backend: 'cli-claude', model: null },
  deviation: 'conservative', constraints: true,
  alerts: { needs_input: true, off_plan: false, claimed_unverified: false, stalled: false, limited: false },
  nativeNotifications: true, focus: { minutes: 30, until: null }, staleGoalDays: 7,
  exclude: { projects: [], runtimes: [] },
};

/** Only explicit criteria participate in progress and deviation decisions. */
export function confirmedRequirement(item: RequirementItem): boolean {
  return item.source !== 'fallback' && !item.dropped;
}
export function ledgerNeedsAttention(row: Pick<LedgerDetail,'state' | 'offPlan'>): boolean {
  return row.state === 'needs_input' || row.state === 'review' || row.state === 'running' && row.offPlan.length > 0;
}
