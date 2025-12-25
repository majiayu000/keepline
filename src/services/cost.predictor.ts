/**
 * Cost Prediction Service
 *
 * Predicts future costs based on historical usage patterns.
 * Uses weighted moving average with recent data weighted more heavily.
 */

import { getAllSessions } from '../adapters/claude/scanner.js';

export interface DailyCost {
  date: string; // YYYY-MM-DD
  cost: number;
  tokens: number;
  sessions: number;
}

export interface CostPrediction {
  todayCost: number;
  weekCost: number;
  monthCost: number;
  projectedWeekCost: number;
  projectedMonthCost: number;
  dailyAverage: number;
  weeklyAverage: number;
  trend: 'up' | 'down' | 'stable';
  trendPercent: number;
  cacheSavings: number;
  cacheHitRate: number;
  dailyBreakdown: DailyCost[];
}

/**
 * Get start of today (midnight local time)
 */
function getStartOfToday(): Date {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), now.getDate());
}

/**
 * Get start of this week (Sunday or Monday depending on locale)
 */
function getStartOfWeek(): Date {
  const now = new Date();
  const day = now.getDay();
  const diff = now.getDate() - day + (day === 0 ? -6 : 1); // Adjust for Monday start
  return new Date(now.getFullYear(), now.getMonth(), diff);
}

/**
 * Get start of this month
 */
function getStartOfMonth(): Date {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), 1);
}

/**
 * Calculate days remaining in month
 */
function getDaysRemainingInMonth(): number {
  const now = new Date();
  const lastDay = new Date(now.getFullYear(), now.getMonth() + 1, 0);
  return lastDay.getDate() - now.getDate();
}

/**
 * Calculate days remaining in week
 */
function getDaysRemainingInWeek(): number {
  const now = new Date();
  const day = now.getDay();
  return day === 0 ? 0 : 7 - day; // Sunday = 0
}

/**
 * Get cost prediction based on historical data
 */
export async function getCostPrediction(): Promise<CostPrediction> {
  const sessions = await getAllSessions();
  const now = new Date();
  const todayStart = getStartOfToday();
  const weekStart = getStartOfWeek();
  const monthStart = getStartOfMonth();

  // Calculate costs by period
  let todayCost = 0;
  let weekCost = 0;
  let monthCost = 0;
  let totalInputTokens = 0;

  // Track daily costs for the last 30 days
  const dailyCosts = new Map<string, DailyCost>();
  const thirtyDaysAgo = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000);

  for (const session of sessions) {
    if (!session.usageStats) continue;

    const stats = session.usageStats;
    const sessionDate = session.startedAt ? new Date(session.startedAt) : new Date();

    // Aggregate by period
    if (sessionDate >= todayStart) {
      todayCost += stats.totalCost;
    }
    if (sessionDate >= weekStart) {
      weekCost += stats.totalCost;
    }
    if (sessionDate >= monthStart) {
      monthCost += stats.totalCost;
    }

    // Track daily breakdown for last 30 days
    if (sessionDate >= thirtyDaysAgo) {
      const dateKey = sessionDate.toISOString().split('T')[0];
      const existing = dailyCosts.get(dateKey) || { date: dateKey, cost: 0, tokens: 0, sessions: 0 };
      existing.cost += stats.totalCost;
      existing.tokens += stats.totalTokens;
      existing.sessions += 1;
      dailyCosts.set(dateKey, existing);

      // Track total input tokens for cache estimate
      totalInputTokens += stats.totalInputTokens;
    }
  }

  // Calculate cache savings estimate (using a heuristic)
  // Cache read tokens cost 0.1x, so savings = cacheReadTokens * 0.9 * price
  const cacheHitRate = totalInputTokens > 0 ? 0.3 : 0; // Rough estimate
  const cacheSavings = totalInputTokens * cacheHitRate * 0.9 * 0.003; // Rough estimate at $3/M

  // Sort daily costs and convert to array
  const dailyBreakdown = Array.from(dailyCosts.values())
    .sort((a, b) => a.date.localeCompare(b.date));

  // Calculate averages
  const daysWithData = dailyBreakdown.length;
  const dailyAverage = daysWithData > 0
    ? dailyBreakdown.reduce((sum, d) => sum + d.cost, 0) / daysWithData
    : 0;

  // Weekly average (last 4 weeks)
  const fourWeeksAgo = new Date(now.getTime() - 28 * 24 * 60 * 60 * 1000);
  const recentDays = dailyBreakdown.filter(d => new Date(d.date) >= fourWeeksAgo);
  const weeklyAverage = recentDays.length > 0
    ? recentDays.reduce((sum, d) => sum + d.cost, 0) / Math.ceil(recentDays.length / 7)
    : 0;

  // Calculate trend (compare last 7 days to previous 7 days)
  const lastWeekDays = dailyBreakdown.slice(-7);
  const prevWeekDays = dailyBreakdown.slice(-14, -7);

  const lastWeekAvg = lastWeekDays.length > 0
    ? lastWeekDays.reduce((sum, d) => sum + d.cost, 0) / lastWeekDays.length
    : 0;
  const prevWeekAvg = prevWeekDays.length > 0
    ? prevWeekDays.reduce((sum, d) => sum + d.cost, 0) / prevWeekDays.length
    : lastWeekAvg;

  let trend: 'up' | 'down' | 'stable' = 'stable';
  let trendPercent = 0;

  if (prevWeekAvg > 0) {
    trendPercent = ((lastWeekAvg - prevWeekAvg) / prevWeekAvg) * 100;
    if (trendPercent > 10) {
      trend = 'up';
    } else if (trendPercent < -10) {
      trend = 'down';
    }
  }

  // Project future costs using weighted average
  // Weight recent days more heavily
  const weights = [1, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2]; // Last day has highest weight
  const weightedDays = lastWeekDays.slice(-7);
  let weightedSum = 0;
  let weightTotal = 0;

  weightedDays.forEach((day, i) => {
    const weight = weights[Math.min(i, weights.length - 1)];
    weightedSum += day.cost * weight;
    weightTotal += weight;
  });

  const projectedDailyAvg = weightTotal > 0 ? weightedSum / weightTotal : dailyAverage;

  // Project week and month costs
  const daysRemainingWeek = getDaysRemainingInWeek();
  const daysRemainingMonth = getDaysRemainingInMonth();

  const projectedWeekCost = weekCost + (projectedDailyAvg * daysRemainingWeek);
  const projectedMonthCost = monthCost + (projectedDailyAvg * daysRemainingMonth);

  return {
    todayCost,
    weekCost,
    monthCost,
    projectedWeekCost,
    projectedMonthCost,
    dailyAverage,
    weeklyAverage,
    trend,
    trendPercent,
    cacheSavings,
    cacheHitRate,
    dailyBreakdown: dailyBreakdown.slice(-30), // Last 30 days
  };
}

/**
 * Get cost summary for a specific date range
 */
export async function getCostForDateRange(
  startDate: Date,
  endDate: Date
): Promise<{ cost: number; tokens: number; sessions: number }> {
  const sessions = await getAllSessions();

  let cost = 0;
  let tokens = 0;
  let sessionCount = 0;

  for (const session of sessions) {
    if (!session.usageStats) continue;

    const sessionDate = session.startedAt ? new Date(session.startedAt) : new Date();

    if (sessionDate >= startDate && sessionDate <= endDate) {
      cost += session.usageStats.totalCost;
      tokens += session.usageStats.totalTokens;
      sessionCount += 1;
    }
  }

  return { cost, tokens, sessions: sessionCount };
}
