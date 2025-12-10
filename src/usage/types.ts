/**
 * Token usage and cost tracking types
 */

/** Token usage for a single API call */
export interface TokenUsage {
  inputTokens: number
  outputTokens: number
}

/** Aggregated usage statistics */
export interface UsageStats {
  totalInputTokens: number
  totalOutputTokens: number
  totalTokens: number
  totalCost: number // in USD
  apiCalls: number
  modelBreakdown: ModelUsage[]
}

/** Usage per model */
export interface ModelUsage {
  model: string
  inputTokens: number
  outputTokens: number
  cost: number
  calls: number
}

/** Pricing per million tokens */
export interface ModelPricing {
  inputPerMillion: number
  outputPerMillion: number
}

/** Pricing configuration */
export type PricingConfig = Record<string, ModelPricing>
