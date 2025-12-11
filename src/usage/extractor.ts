/**
 * Token usage extraction from Claude JSONL entries
 *
 * Claude's usage structure includes cache tokens:
 * - input_tokens: base input tokens
 * - cache_creation_input_tokens: tokens written to cache (1.25x cost)
 * - cache_read_input_tokens: tokens read from cache (0.1x cost)
 * - output_tokens: output tokens
 */

import type { ClaudeEntry, ClaudeAssistantEntry } from '../claude/types.js'
import type { UsageStats, ModelUsage } from './types.js'
import { getModelPricing, calculateCost, calculateCostWithCache } from './pricing.js'

/** Claude's extended usage structure with cache tokens */
interface ClaudeUsage {
  input_tokens: number
  output_tokens: number
  cache_creation_input_tokens?: number
  cache_read_input_tokens?: number
}

/** Extracted usage info from a single entry */
export interface EntryUsage {
  model: string
  inputTokens: number
  outputTokens: number
  cacheCreationTokens: number
  cacheReadTokens: number
  timestamp: string
}

/** Check if entry is an assistant entry with usage data */
function isAssistantEntryWithUsage(
  entry: ClaudeEntry
): entry is ClaudeAssistantEntry & { message: { usage: ClaudeUsage } } {
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
      const u = entry.message.usage
      usage.push({
        model: entry.message.model,
        inputTokens: u.input_tokens,
        outputTokens: u.output_tokens,
        cacheCreationTokens: u.cache_creation_input_tokens || 0,
        cacheReadTokens: u.cache_read_input_tokens || 0,
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
    // Total input includes base + cache creation + cache read tokens
    const effectiveInputTokens = usage.inputTokens + usage.cacheCreationTokens + usage.cacheReadTokens
    totalInputTokens += effectiveInputTokens
    totalOutputTokens += usage.outputTokens

    // Calculate cost with cache pricing
    const pricing = getModelPricing(usage.model)
    const cost = calculateCostWithCache(
      usage.inputTokens,
      usage.outputTokens,
      usage.cacheCreationTokens,
      usage.cacheReadTokens,
      pricing
    )
    totalCost += cost

    // Update model breakdown
    const existing = modelMap.get(usage.model)
    if (existing) {
      existing.inputTokens += effectiveInputTokens
      existing.outputTokens += usage.outputTokens
      existing.cost += cost
      existing.calls += 1
    } else {
      modelMap.set(usage.model, {
        model: usage.model,
        inputTokens: effectiveInputTokens,
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
