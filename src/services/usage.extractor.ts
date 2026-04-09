/**
 * Token usage extraction from Claude JSONL entries
 *
 * Claude's usage structure includes cache tokens:
 * - input_tokens: base input tokens
 * - cache_creation_input_tokens: tokens written to cache (1.25x cost)
 * - cache_read_input_tokens: tokens read from cache (0.1x cost)
 * - output_tokens: output tokens
 */

import type { ClaudeEntry, ClaudeAssistantEntry } from '../adapters/claude/types.js'
import type { UsageStats, ModelUsage } from './usage.types.js'
import { getModelPricing } from './usage.pricing.js'

/** Claude's extended usage structure with cache tokens */
interface ClaudeUsage {
  input_tokens: number
  output_tokens: number
  cache_creation_input_tokens?: number
  cache_read_input_tokens?: number
}

type UsageCostModel = {
  inputPerToken: number
  outputPerToken: number
}

type UsageAccumulator = {
  totalInputTokens: number
  totalOutputTokens: number
  totalCost: number
  apiCalls: number
  modelMap: Map<string, ModelUsage>
  costPerTokenByModel: Map<string, UsageCostModel>
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

export function createUsageAccumulator(): UsageAccumulator {
  return {
    totalInputTokens: 0,
    totalOutputTokens: 0,
    totalCost: 0,
    apiCalls: 0,
    modelMap: new Map<string, ModelUsage>(),
    costPerTokenByModel: new Map<string, UsageCostModel>(),
  }
}

export function addUsageToAccumulator(
  accumulator: UsageAccumulator,
  model: string,
  usage: ClaudeUsage
): void {
  const cacheCreationTokens = usage.cache_creation_input_tokens || 0
  const cacheReadTokens = usage.cache_read_input_tokens || 0
  const effectiveInputTokens = usage.input_tokens + cacheCreationTokens + cacheReadTokens

  accumulator.apiCalls += 1
  accumulator.totalInputTokens += effectiveInputTokens
  accumulator.totalOutputTokens += usage.output_tokens

  let costModel = accumulator.costPerTokenByModel.get(model)
  if (!costModel) {
    const pricing = getModelPricing(model)
    costModel = {
      inputPerToken: pricing.inputPerMillion / 1_000_000,
      outputPerToken: pricing.outputPerMillion / 1_000_000,
    }
    accumulator.costPerTokenByModel.set(model, costModel)
  }

  // Cache pricing: cache write 1.25x, cache read 0.1x input token price.
  const cost =
    usage.input_tokens * costModel.inputPerToken +
    cacheCreationTokens * costModel.inputPerToken * 1.25 +
    cacheReadTokens * costModel.inputPerToken * 0.1 +
    usage.output_tokens * costModel.outputPerToken

  accumulator.totalCost += cost

  const existing = accumulator.modelMap.get(model)
  if (existing) {
    existing.inputTokens += effectiveInputTokens
    existing.outputTokens += usage.output_tokens
    existing.cost += cost
    existing.calls += 1
  } else {
    accumulator.modelMap.set(model, {
      model,
      inputTokens: effectiveInputTokens,
      outputTokens: usage.output_tokens,
      cost,
      calls: 1,
    })
  }
}

export function usageStatsFromAccumulator(accumulator: UsageAccumulator): UsageStats {
  if (accumulator.apiCalls === 0) {
    return {
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalTokens: 0,
      totalCost: 0,
      apiCalls: 0,
      modelBreakdown: [],
    }
  }

  return {
    totalInputTokens: accumulator.totalInputTokens,
    totalOutputTokens: accumulator.totalOutputTokens,
    totalTokens: accumulator.totalInputTokens + accumulator.totalOutputTokens,
    totalCost: accumulator.totalCost,
    apiCalls: accumulator.apiCalls,
    modelBreakdown: Array.from(accumulator.modelMap.values()),
  }
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
  const costPerTokenByModel = new Map<string, UsageCostModel>()
  const modelMap = new Map<string, ModelUsage>()
  let totalInputTokens = 0
  let totalOutputTokens = 0
  let totalCost = 0
  let apiCalls = 0

  for (const entry of entries) {
    if (!isAssistantEntryWithUsage(entry)) {
      continue
    }

    apiCalls += 1
    const usage = entry.message.usage
    const model = entry.message.model
    const cacheCreationTokens = usage.cache_creation_input_tokens || 0
    const cacheReadTokens = usage.cache_read_input_tokens || 0
    const effectiveInputTokens = usage.input_tokens + cacheCreationTokens + cacheReadTokens
    totalInputTokens += effectiveInputTokens
    totalOutputTokens += usage.output_tokens

    let costModel = costPerTokenByModel.get(model)
    if (!costModel) {
      const pricing = getModelPricing(model)
      costModel = {
        inputPerToken: pricing.inputPerMillion / 1_000_000,
        outputPerToken: pricing.outputPerMillion / 1_000_000,
      }
      costPerTokenByModel.set(model, costModel)
    }

    const cost =
      usage.input_tokens * costModel.inputPerToken +
      cacheCreationTokens * costModel.inputPerToken * 1.25 +
      cacheReadTokens * costModel.inputPerToken * 0.1 +
      usage.output_tokens * costModel.outputPerToken
    totalCost += cost

    const existing = modelMap.get(model)
    if (existing) {
      existing.inputTokens += effectiveInputTokens
      existing.outputTokens += usage.output_tokens
      existing.cost += cost
      existing.calls += 1
    } else {
      modelMap.set(model, {
        model,
        inputTokens: effectiveInputTokens,
        outputTokens: usage.output_tokens,
        cost,
        calls: 1,
      })
    }
  }

  if (apiCalls === 0) {
    return {
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalTokens: 0,
      totalCost: 0,
      apiCalls: 0,
      modelBreakdown: [],
    }
  }

  return {
    totalInputTokens,
    totalOutputTokens,
    totalTokens: totalInputTokens + totalOutputTokens,
    totalCost,
    apiCalls,
    modelBreakdown: Array.from(modelMap.values()),
  }
}
