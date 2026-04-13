/**
 * Token usage extraction from Claude JSONL entries
 *
 * Claude's usage structure includes cache tokens:
 * - input_tokens: base input tokens
 * - cache_creation_input_tokens: tokens written to cache (1.25x cost)
 * - cache_read_input_tokens: tokens read from cache (0.1x cost)
 * - output_tokens: output tokens
 */

import type { ClaudeEntry } from '../adapters/claude/types.js'
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

type UsageBucket = {
  usage: ModelUsage
  costModel: UsageCostModel
}

type UsageAccumulator = {
  totalInputTokens: number
  totalOutputTokens: number
  totalCost: number
  apiCalls: number
  buckets: Map<string, UsageBucket>
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
    buckets: new Map<string, UsageBucket>(),
  }
}

const resolvedUsageCostModels = new Map<string, UsageCostModel>()

function getUsageCostModel(model: string): UsageCostModel {
  const cached = resolvedUsageCostModels.get(model)
  if (cached) {
    return cached
  }

  const pricing = getModelPricing(model)
  const costModel = {
    inputPerToken: pricing.inputPerMillion / 1_000_000,
    outputPerToken: pricing.outputPerMillion / 1_000_000,
  }
  resolvedUsageCostModels.set(model, costModel)
  return costModel
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

  let bucket = accumulator.buckets.get(model)
  if (!bucket) {
    bucket = {
      usage: {
        model,
        inputTokens: 0,
        outputTokens: 0,
        cost: 0,
        calls: 0,
      },
      costModel: getUsageCostModel(model),
    }
    accumulator.buckets.set(model, bucket)
  }

  // Cache pricing: cache write 1.25x, cache read 0.1x input token price.
  const cost =
    usage.input_tokens * bucket.costModel.inputPerToken +
    cacheCreationTokens * bucket.costModel.inputPerToken * 1.25 +
    cacheReadTokens * bucket.costModel.inputPerToken * 0.1 +
    usage.output_tokens * bucket.costModel.outputPerToken

  accumulator.totalCost += cost

  bucket.usage.inputTokens += effectiveInputTokens
  bucket.usage.outputTokens += usage.output_tokens
  bucket.usage.cost += cost
  bucket.usage.calls += 1
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

  const modelBreakdown = new Array<ModelUsage>(accumulator.buckets.size)
  let bucketIndex = 0
  for (const bucket of accumulator.buckets.values()) {
    modelBreakdown[bucketIndex++] = bucket.usage
  }

  return {
    totalInputTokens: accumulator.totalInputTokens,
    totalOutputTokens: accumulator.totalOutputTokens,
    totalTokens: accumulator.totalInputTokens + accumulator.totalOutputTokens,
    totalCost: accumulator.totalCost,
    apiCalls: accumulator.apiCalls,
    modelBreakdown,
  }
}

/** Extract usage data from JSONL entries */
export function extractUsageFromEntries(entries: ClaudeEntry[]): EntryUsage[] {
  const usage: EntryUsage[] = []

  for (const entry of entries) {
    if (entry.type !== 'assistant') {
      continue
    }

    const message = entry.message
    const u = message.usage as ClaudeUsage | undefined
    if (
      !u ||
      typeof u.input_tokens !== 'number' ||
      typeof u.output_tokens !== 'number'
    ) {
      continue
    }

    usage.push({
      model: message.model,
      inputTokens: u.input_tokens,
      outputTokens: u.output_tokens,
      cacheCreationTokens: u.cache_creation_input_tokens || 0,
      cacheReadTokens: u.cache_read_input_tokens || 0,
      timestamp: entry.timestamp,
    })
  }

  return usage
}

/** Aggregate usage stats from JSONL entries */
export function aggregateUsageStats(entries: ClaudeEntry[]): UsageStats {
  const buckets = new Map<string, UsageBucket>()
  let totalInputTokens = 0
  let totalOutputTokens = 0
  let totalCost = 0
  let apiCalls = 0

  for (const entry of entries) {
    if (entry.type !== 'assistant') {
      continue
    }

    const message = entry.message
    const usage = message.usage as ClaudeUsage | undefined
    if (
      !usage ||
      typeof usage.input_tokens !== 'number' ||
      typeof usage.output_tokens !== 'number'
    ) {
      continue
    }

    apiCalls += 1
    const model = message.model
    const cacheCreationTokens = usage.cache_creation_input_tokens || 0
    const cacheReadTokens = usage.cache_read_input_tokens || 0
    const effectiveInputTokens = usage.input_tokens + cacheCreationTokens + cacheReadTokens
    totalInputTokens += effectiveInputTokens
    totalOutputTokens += usage.output_tokens

    let bucket = buckets.get(model)
    if (!bucket) {
      bucket = {
        usage: {
          model,
          inputTokens: 0,
          outputTokens: 0,
          cost: 0,
          calls: 0,
        },
        costModel: getUsageCostModel(model),
      }
      buckets.set(model, bucket)
    }

    const cost =
      usage.input_tokens * bucket.costModel.inputPerToken +
      cacheCreationTokens * bucket.costModel.inputPerToken * 1.25 +
      cacheReadTokens * bucket.costModel.inputPerToken * 0.1 +
      usage.output_tokens * bucket.costModel.outputPerToken
    totalCost += cost

    bucket.usage.inputTokens += effectiveInputTokens
    bucket.usage.outputTokens += usage.output_tokens
    bucket.usage.cost += cost
    bucket.usage.calls += 1
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

  const modelBreakdown = new Array<ModelUsage>(buckets.size)
  let bucketIndex = 0
  for (const bucket of buckets.values()) {
    modelBreakdown[bucketIndex++] = bucket.usage
  }

  return {
    totalInputTokens,
    totalOutputTokens,
    totalTokens: totalInputTokens + totalOutputTokens,
    totalCost,
    apiCalls,
    modelBreakdown,
  }
}
