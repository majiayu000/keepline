import type { AgentClient, SessionStatus } from '../domain/session/index.js';
import type { AggregatedSession } from './session.types.js';

export type AttentionReasonCode =
  | 'waiting_for_human'
  | 'recoverable_lost'
  | 'high_cost'
  | 'stale_activity'
  | 'idle_activity'
  | 'active_session';

export type AttentionSeverity = 'critical' | 'warning' | 'info';

export type RecommendedAction = 'review' | 'recover' | 'monitor' | 'resume' | 'none';

export interface AttentionReason {
  code: AttentionReasonCode;
  severity: AttentionSeverity;
  message: string;
  score: number;
}

export interface AttentionQueueItem {
  rank: number;
  sessionId: string;
  client: AgentClient;
  status: SessionStatus;
  title: string;
  directory: string;
  lastActiveAt: Date;
  score: number;
  reasons: AttentionReason[];
  recommendedAction: RecommendedAction;
  processRunning: boolean;
  usageCost?: number;
}

export interface AttentionOverviewStats {
  totalCandidates: number;
  needingAttention: number;
  critical: number;
  warning: number;
}

export interface AttentionOverview {
  generatedAt: Date;
  items: AttentionQueueItem[];
  stats: AttentionOverviewStats;
}

export interface AttentionOverviewOptions {
  includeCompleted?: boolean;
  limit?: number;
  now?: Date;
  highCostThreshold?: number;
  staleHours?: number;
}

export const DEFAULT_ATTENTION_LIMIT = 20;
export const MAX_ATTENTION_LIMIT = 100;
export const DEFAULT_HIGH_COST_THRESHOLD = 1;
export const DEFAULT_STALE_HOURS = 24;

const WAITING_SCORE = 1000;
const LOST_SCORE = 850;
const HIGH_COST_SCORE = 600;
const STALE_SCORE = 350;
const IDLE_SCORE = 150;
const RUNNING_SCORE = 25;

export function buildAttentionOverview(
  sessions: AggregatedSession[],
  options: AttentionOverviewOptions = {}
): AttentionOverview {
  const now = options.now ?? new Date();
  const limit = clampLimit(options.limit);
  const highCostThreshold = options.highCostThreshold ?? DEFAULT_HIGH_COST_THRESHOLD;
  const staleHours = options.staleHours ?? DEFAULT_STALE_HOURS;
  const staleCutoffMs = now.getTime() - staleHours * 60 * 60 * 1000;
  const candidates = sessions.filter(
    (session) => options.includeCompleted || session.status !== 'completed'
  );
  const rankedItems = candidates
    .map((session) => buildAttentionItem(session, {
      highCostThreshold,
      staleCutoffMs,
    }))
    .sort(compareAttentionItems)
    .map((item, index) => ({ ...item, rank: index + 1 }));
  const items = rankedItems.slice(0, limit);

  return {
    generatedAt: now,
    items,
    stats: {
      totalCandidates: candidates.length,
      needingAttention: rankedItems.filter(hasCriticalOrWarningReason).length,
      critical: rankedItems.filter(hasCriticalReason).length,
      warning: rankedItems.filter(hasWarningReason).length,
    },
  };
}

function buildAttentionItem(
  session: AggregatedSession,
  options: {
    highCostThreshold: number;
    staleCutoffMs: number;
  }
): AttentionQueueItem {
  const reasons: AttentionReason[] = [];

  if (session.status === 'waiting') {
    reasons.push({
      code: 'waiting_for_human',
      severity: 'critical',
      message: 'Waiting for human input',
      score: WAITING_SCORE,
    });
  }

  if (session.status === 'lost') {
    reasons.push({
      code: 'recoverable_lost',
      severity: 'critical',
      message: 'Session is lost and may be recoverable',
      score: LOST_SCORE,
    });
  }

  const usageCost = session.usageStats?.totalCost;
  if (usageCost != null && usageCost >= options.highCostThreshold) {
    const costBonus = Math.min(
      Math.floor((usageCost / Math.max(options.highCostThreshold, 0.01)) * 50),
      200
    );
    reasons.push({
      code: 'high_cost',
      severity: 'warning',
      message: `Session cost is $${usageCost.toFixed(2)}`,
      score: HIGH_COST_SCORE + costBonus,
    });
  }

  const stale = session.lastActiveAt.getTime() < options.staleCutoffMs;
  if ((session.status === 'running' || session.status === 'idle') && stale) {
    reasons.push({
      code: 'stale_activity',
      severity: 'warning',
      message: 'No recent activity',
      score: STALE_SCORE,
    });
  }

  if (session.status === 'idle') {
    reasons.push({
      code: 'idle_activity',
      severity: 'info',
      message: 'Session is idle',
      score: IDLE_SCORE,
    });
  }

  if (session.status === 'running') {
    reasons.push({
      code: 'active_session',
      severity: 'info',
      message: 'Session is running',
      score: RUNNING_SCORE,
    });
  }

  return {
    rank: 0,
    sessionId: session.sessionId,
    client: session.client,
    status: session.status,
    title: session.title,
    directory: session.directory,
    lastActiveAt: session.lastActiveAt,
    score: getPriorityScore(reasons),
    reasons,
    recommendedAction: getRecommendedAction(reasons),
    processRunning: session.processRunning,
    usageCost,
  };
}

function getPriorityScore(reasons: AttentionReason[]): number {
  if (reasons.length === 0) return 0;
  return Math.max(...reasons.map((reason) => reason.score));
}

function clampLimit(limit: number | undefined): number {
  if (limit == null) return DEFAULT_ATTENTION_LIMIT;
  if (!Number.isFinite(limit) || limit <= 0) return DEFAULT_ATTENTION_LIMIT;
  return Math.min(Math.floor(limit), MAX_ATTENTION_LIMIT);
}

function getRecommendedAction(reasons: AttentionReason[]): RecommendedAction {
  if (reasons.some((reason) => reason.code === 'waiting_for_human')) return 'review';
  if (reasons.some((reason) => reason.code === 'recoverable_lost')) return 'recover';
  if (reasons.some((reason) => reason.code === 'high_cost')) return 'review';
  if (reasons.some((reason) => reason.code === 'stale_activity')) return 'review';
  if (reasons.some((reason) => reason.code === 'idle_activity')) return 'monitor';
  if (reasons.some((reason) => reason.code === 'active_session')) return 'monitor';
  return 'none';
}

function compareAttentionItems(a: AttentionQueueItem, b: AttentionQueueItem): number {
  const scoreComparison = b.score - a.score;
  if (scoreComparison !== 0) return scoreComparison;
  const activityComparison = b.lastActiveAt.getTime() - a.lastActiveAt.getTime();
  if (activityComparison !== 0) return activityComparison;
  return a.sessionId.localeCompare(b.sessionId);
}

function hasCriticalReason(item: AttentionQueueItem): boolean {
  return item.reasons.some((reason) => reason.severity === 'critical');
}

function hasWarningReason(item: AttentionQueueItem): boolean {
  return item.reasons.some((reason) => reason.severity === 'warning');
}

function hasCriticalOrWarningReason(item: AttentionQueueItem): boolean {
  return item.reasons.some((reason) => reason.severity !== 'info');
}
