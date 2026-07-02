/**
 * Tests for pricing module
 */

import { beforeEach, describe, test, expect } from 'bun:test'
import {
  DEFAULT_PRICING,
  FALLBACK_PRICING,
  calculateCost,
  getModelPricing,
  pricingConfigFromLiteLLMData,
  resetPricingForTests,
} from '../services/usage.pricing.js'
import { logger } from '../lib/logger.js'

beforeEach(() => {
  resetPricingForTests(DEFAULT_PRICING)
})

describe('getModelPricing', () => {
  test('returns exact match pricing for known model', () => {
    const pricing = getModelPricing('claude-3-5-sonnet-20241022')
    expect(pricing.inputPerMillion).toBe(3.0)
    expect(pricing.outputPerMillion).toBe(15.0)
  })

  test('returns opus pricing for opus model variants', () => {
    const pricing = getModelPricing('claude-opus-4-something')
    expect(pricing.inputPerMillion).toBeGreaterThanOrEqual(5.0)
    expect(pricing.outputPerMillion).toBeGreaterThanOrEqual(25.0)
  })

  test('returns haiku pricing for haiku model variants', () => {
    const pricing = getModelPricing('claude-3-haiku-latest')
    // Haiku should be cheaper than Sonnet
    expect(pricing.inputPerMillion).toBeLessThan(3.0)
    expect(pricing.outputPerMillion).toBeLessThan(15.0)
  })

  test('returns sonnet pricing for sonnet model variants', () => {
    const pricing = getModelPricing('claude-sonnet-unknown')
    expect(pricing.inputPerMillion).toBe(3.0)
    expect(pricing.outputPerMillion).toBe(15.0)
  })

  test('returns fallback pricing for completely unknown model', () => {
    const pricing = getModelPricing('unknown-model-xyz')
    expect(pricing).toEqual(FALLBACK_PRICING)
  })

  test('keeps non-Anthropic LiteLLM pricing entries', () => {
    const pricing = pricingConfigFromLiteLLMData({
      'openai/gpt-4o-mini': {
        litellm_provider: 'openai',
        input_cost_per_token: 0.00000015,
        output_cost_per_token: 0.0000006,
      },
      'claude/claude-3-5-sonnet-20241022': {
        litellm_provider: 'anthropic',
        input_cost_per_token: 0.000003,
        output_cost_per_token: 0.000015,
      },
      'openai/free-embedding': {
        litellm_provider: 'openai',
        input_cost_per_token: 0,
      },
    })

    expect(pricing['openai/gpt-4o-mini']).toEqual({
      inputPerMillion: 0.15,
      outputPerMillion: 0.6,
    })
    expect(pricing['claude/claude-3-5-sonnet-20241022']).toEqual({
      inputPerMillion: 3,
      outputPerMillion: 15,
    })
    expect(pricing['openai/free-embedding']).toBeUndefined()
  })

  test('logs fallback pricing for unknown models once per model', () => {
    const warnings: Array<{ message: string; data: unknown }> = []
    const originalWarn = logger.warn.bind(logger)
    logger.warn = ((message: string, data?: unknown) => {
      warnings.push({ message, data })
    }) as typeof logger.warn

    try {
      getModelPricing('unknown-model-abc')
      getModelPricing('unknown-model-abc')
      getModelPricing('unknown-model-def')
    } finally {
      logger.warn = originalWarn
    }

    expect(warnings).toHaveLength(2)
    expect(warnings[0]).toEqual({
      message: 'Using fallback pricing for unknown model',
      data: { model: 'unknown-model-abc' },
    })
    expect(warnings[1]).toEqual({
      message: 'Using fallback pricing for unknown model',
      data: { model: 'unknown-model-def' },
    })
  })
})

describe('calculateCost', () => {
  test('calculates cost correctly for Claude 3.5 Sonnet', () => {
    const pricing = { inputPerMillion: 3.0, outputPerMillion: 15.0 }
    // 1000 input tokens + 500 output tokens
    const cost = calculateCost(1000, 500, pricing)
    // (1000/1M * 3) + (500/1M * 15) = 0.003 + 0.0075 = 0.0105
    expect(cost).toBeCloseTo(0.0105, 6)
  })

  test('calculates cost correctly for Claude 3 Opus', () => {
    const pricing = { inputPerMillion: 15.0, outputPerMillion: 75.0 }
    // 10000 input tokens + 2000 output tokens
    const cost = calculateCost(10000, 2000, pricing)
    // (10000/1M * 15) + (2000/1M * 75) = 0.15 + 0.15 = 0.30
    expect(cost).toBeCloseTo(0.30, 6)
  })

  test('calculates cost correctly for Claude 3 Haiku', () => {
    const pricing = { inputPerMillion: 0.25, outputPerMillion: 1.25 }
    // 100000 input tokens + 50000 output tokens
    const cost = calculateCost(100000, 50000, pricing)
    // (100000/1M * 0.25) + (50000/1M * 1.25) = 0.025 + 0.0625 = 0.0875
    expect(cost).toBeCloseTo(0.0875, 6)
  })

  test('returns 0 for zero tokens', () => {
    const pricing = { inputPerMillion: 3.0, outputPerMillion: 15.0 }
    const cost = calculateCost(0, 0, pricing)
    expect(cost).toBe(0)
  })

  test('handles large token counts', () => {
    const pricing = { inputPerMillion: 3.0, outputPerMillion: 15.0 }
    // 1 million input + 1 million output
    const cost = calculateCost(1_000_000, 1_000_000, pricing)
    // 3 + 15 = 18
    expect(cost).toBeCloseTo(18.0, 6)
  })
})
