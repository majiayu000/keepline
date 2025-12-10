/**
 * Token usage extraction from Claude JSONL entries
 */

import type { ClaudeEntry, ClaudeAssistantEntry } from '../claude/types.js'
import type { UsageStats, ModelUsage } from './types.js'
import { getModelPricing, calculateCost } from './pricing.js'

/** Extracted usage info from a single entry */
export interface EntryUsage {
  model: string
  inputTokens: number
  outputTokens: number
  timestamp: string
}

/** Check if entry is an assistant entry with usage data */
function isAssistantEntryWithUsage(
  entry: ClaudeEntry
): entry is ClaudeAssistantEntry & { message: { usage: { input_tokens: number; output_tokens: number } } } {
  return (
    entry.type === 'assistant' &&
    'message' in entry &&
    entry.message.usage !== undefined &&
    typeof entry.message.usage.input_tokens === 'number' &&
    typeof entry.message.usage.output_tokens === 'number'
  )
}

/** Extract usage data from JSONL entries */
export function extractUsageFromEntries(entries: ClaudeEntry[]): EntryUsage[] {
  const usage: EntryUsage[] = []

  for (const entry of entries) {
    if (isAssistantEntryWithUsage(entry)) {
      usage.push({
        model: entry.message.model,
        inputTokens: entry.message.usage.input_tokens,
        outputTokens: entry.message.usage.output_tokens,
        timestamp: entry.timestamp,
      })
    }
  }

  return usage
}

/** Aggregate usage stats from JSONL entries */
export function aggregateUsageStats(entries: ClaudeEntry[]): UsageStats {
  const usageList = extractUsageFromEntries(entries)

  if (usageList.length === 0) {
    return {
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalTokens: 0,
      totalCost: 0,
      apiCalls: 0,
      modelBreakdown: [],
    }
  }

  // Aggregate by model
  const modelMap = new Map<string, ModelUsage>()

  let totalInputTokens = 0
  let totalOutputTokens = 0
  let totalCost = 0

  for (const usage of usageList) {
    totalInputTokens += usage.inputTokens
    totalOutputTokens += usage.outputTokens

    const pricing = getModelPricing(usage.model)
    const cost = calculateCost(usage.inputTokens, usage.outputTokens, pricing)
    totalCost += cost

    // Update model breakdown
    const existing = modelMap.get(usage.model)
    if (existing) {
      existing.inputTokens += usage.inputTokens
      existing.outputTokens += usage.outputTokens
      existing.cost += cost
      existing.calls += 1
    } else {
      modelMap.set(usage.model, {
        model: usage.model,
        inputTokens: usage.inputTokens,
        outputTokens: usage.outputTokens,
        cost,
        calls: 1,
      })
    }
  }

  return {
    totalInputTokens,
    totalOutputTokens,
    totalTokens: totalInputTokens + totalOutputTokens,
    totalCost,
    apiCalls: usageList.length,
    modelBreakdown: Array.from(modelMap.values()),
  }
}
