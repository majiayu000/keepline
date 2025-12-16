/**
 * Model pricing configuration
 * Fetches prices from LiteLLM's model_prices_and_context_window.json
 */

import type { PricingConfig, ModelPricing } from './types.js'
import { logger } from '../utils/logger.js'

const LITELLM_PRICING_URL =
  'https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json'

/** Cached pricing data */
let cachedPricing: PricingConfig | null = null
let cacheTime: number = 0
const CACHE_TTL = 24 * 60 * 60 * 1000 // 24 hours

/** LiteLLM model entry structure */
interface LiteLLMModelEntry {
  input_cost_per_token?: number
  output_cost_per_token?: number
  litellm_provider?: string
  mode?: string
}

/** Fallback pricing for unknown models (Claude 3.5 Sonnet rates) */
export const FALLBACK_PRICING: ModelPricing = {
  inputPerMillion: 3.0,
  outputPerMillion: 15.0,
}

/** Default pricing as fallback when fetch fails */
export const DEFAULT_PRICING: PricingConfig = {
  // Claude 3.5 Sonnet
  'claude-3-5-sonnet-20241022': { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  'claude-3-5-sonnet-20240620': { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  // Claude 3.5 Haiku
  'claude-3-5-haiku-20241022': { inputPerMillion: 1.0, outputPerMillion: 5.0 },
  // Claude 3 Opus
  'claude-3-opus-20240229': { inputPerMillion: 15.0, outputPerMillion: 75.0 },
  // Claude 3 Sonnet
  'claude-3-sonnet-20240229': { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  // Claude 3 Haiku
  'claude-3-haiku-20240307': { inputPerMillion: 0.25, outputPerMillion: 1.25 },
  // Claude Sonnet 4
  'claude-sonnet-4-20250514': { inputPerMillion: 3.0, outputPerMillion: 15.0 },
  // Claude Opus 4
  'claude-opus-4-20250514': { inputPerMillion: 15.0, outputPerMillion: 75.0 },
  // Claude Opus 4.5
  'claude-opus-4-5-20251101': { inputPerMillion: 5.0, outputPerMillion: 25.0 },
  // Claude Sonnet 4.5
  'claude-sonnet-4-5-20250929': { inputPerMillion: 3.0, outputPerMillion: 15.0 },
}

/** Fetch pricing from LiteLLM */
async function fetchLiteLLMPricing(): Promise<PricingConfig> {
  try {
    const response = await fetch(LITELLM_PRICING_URL)
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`)
    }

    const data = (await response.json()) as Record<string, LiteLLMModelEntry>
    const pricing: PricingConfig = {}

    // Extract Claude models from Anthropic provider
    for (const [modelId, entry] of Object.entries(data)) {
      // Only process Anthropic models
      if (
        entry.litellm_provider === 'anthropic' &&
        entry.input_cost_per_token !== undefined &&
        entry.output_cost_per_token !== undefined
      ) {
        // Convert per-token cost to per-million cost
        pricing[modelId] = {
          inputPerMillion: entry.input_cost_per_token * 1_000_000,
          outputPerMillion: entry.output_cost_per_token * 1_000_000,
        }
      }
    }

    return pricing
  } catch (error) {
    logger.warn('Failed to fetch LiteLLM pricing, using defaults', error)
    return DEFAULT_PRICING
  }
}

/** Get pricing config (with caching) */
export async function getPricingConfig(): Promise<PricingConfig> {
  const now = Date.now()

  if (cachedPricing && now - cacheTime < CACHE_TTL) {
    return cachedPricing
  }

  cachedPricing = await fetchLiteLLMPricing()
  cacheTime = now

  return cachedPricing
}

/** Get pricing for a model (sync version using cache or defaults) */
export function getModelPricing(model: string): ModelPricing {
  // Use cached pricing if available
  const pricing = cachedPricing || DEFAULT_PRICING

  // Try exact match first
  if (model in pricing) {
    return pricing[model]
  }

  // Try with claude/ prefix (LiteLLM format)
  const litellmKey = `claude/${model}`
  if (litellmKey in pricing) {
    return pricing[litellmKey]
  }

  // Try prefix match for versioned models
  for (const [key, value] of Object.entries(pricing)) {
    const baseKey = key.replace('claude/', '')
    if (model.startsWith(baseKey.split('-').slice(0, -1).join('-'))) {
      return value
    }
  }

  // Check for model family
  if (model.includes('opus-4-5') || model.includes('opus-4.5')) {
    return pricing['claude/claude-opus-4-5-20251101'] || pricing['claude-opus-4-5-20251101'] || { inputPerMillion: 5.0, outputPerMillion: 25.0 }
  }
  if (model.includes('opus')) {
    return pricing['claude/claude-3-opus-20240229'] || pricing['claude-3-opus-20240229'] || { inputPerMillion: 15.0, outputPerMillion: 75.0 }
  }
  if (model.includes('haiku')) {
    return pricing['claude/claude-3-5-haiku-20241022'] || pricing['claude-3-5-haiku-20241022'] || { inputPerMillion: 1.0, outputPerMillion: 5.0 }
  }
  if (model.includes('sonnet')) {
    return pricing['claude/claude-3-5-sonnet-20241022'] || pricing['claude-3-5-sonnet-20241022'] || { inputPerMillion: 3.0, outputPerMillion: 15.0 }
  }

  return FALLBACK_PRICING
}

/** Calculate cost for token usage (without cache) */
export function calculateCost(
  inputTokens: number,
  outputTokens: number,
  pricing: ModelPricing
): number {
  const inputCost = (inputTokens / 1_000_000) * pricing.inputPerMillion
  const outputCost = (outputTokens / 1_000_000) * pricing.outputPerMillion
  return inputCost + outputCost
}

/**
 * Calculate cost with cache token pricing
 *
 * Anthropic cache pricing:
 * - Base input tokens: normal price
 * - Cache creation tokens: 1.25x base price
 * - Cache read tokens: 0.1x base price (90% discount)
 * - Output tokens: normal price
 */
export function calculateCostWithCache(
  baseInputTokens: number,
  outputTokens: number,
  cacheCreationTokens: number,
  cacheReadTokens: number,
  pricing: ModelPricing
): number {
  const baseInputCost = (baseInputTokens / 1_000_000) * pricing.inputPerMillion
  const cacheWriteCost = (cacheCreationTokens / 1_000_000) * pricing.inputPerMillion * 1.25
  const cacheReadCost = (cacheReadTokens / 1_000_000) * pricing.inputPerMillion * 0.1
  const outputCost = (outputTokens / 1_000_000) * pricing.outputPerMillion

  return baseInputCost + cacheWriteCost + cacheReadCost + outputCost
}

/** Initialize pricing (call at startup) */
export async function initPricing(): Promise<void> {
  await getPricingConfig()
}
