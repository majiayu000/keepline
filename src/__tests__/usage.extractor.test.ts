/**
 * Tests for token usage extraction from JSONL data
 */

import { describe, test, expect } from 'bun:test'
import { extractUsageFromEntries, aggregateUsageStats } from '../services/usage.extractor.js'
import type { ClaudeAssistantEntry } from '../adapters/claude/types.js'

// Helper to create a mock assistant entry with usage
function createAssistantEntry(
  model: string,
  inputTokens: number,
  outputTokens: number
): ClaudeAssistantEntry {
  return {
    type: 'assistant',
    uuid: 'test-uuid',
    sessionId: 'test-session',
    cwd: '/test',
    timestamp: new Date().toISOString(),
    message: {
      role: 'assistant',
      model,
      content: [{ type: 'text', text: 'test response' }],
      usage: { input_tokens: inputTokens, output_tokens: outputTokens },
    },
  }
}

// Helper to create entry without usage (some older entries might lack it)
function createAssistantEntryWithoutUsage(model: string): ClaudeAssistantEntry {
  return {
    type: 'assistant',
    uuid: 'test-uuid',
    sessionId: 'test-session',
    cwd: '/test',
    timestamp: new Date().toISOString(),
    message: {
      role: 'assistant',
      model,
      content: [{ type: 'text', text: 'test response' }],
    },
  }
}

describe('extractUsageFromEntries', () => {
  test('extracts usage from single entry', () => {
    const entries = [createAssistantEntry('claude-3-5-sonnet-20241022', 1000, 500)]
    const usage = extractUsageFromEntries(entries)

    expect(usage.length).toBe(1)
    expect(usage[0].model).toBe('claude-3-5-sonnet-20241022')
    expect(usage[0].inputTokens).toBe(1000)
    expect(usage[0].outputTokens).toBe(500)
  })

  test('extracts usage from multiple entries', () => {
    const entries = [
      createAssistantEntry('claude-3-5-sonnet-20241022', 1000, 500),
      createAssistantEntry('claude-3-5-sonnet-20241022', 2000, 1000),
      createAssistantEntry('claude-3-opus-20240229', 500, 200),
    ]
    const usage = extractUsageFromEntries(entries)

    expect(usage.length).toBe(3)
  })

  test('skips entries without usage data', () => {
    const entries = [
      createAssistantEntry('claude-3-5-sonnet-20241022', 1000, 500),
      createAssistantEntryWithoutUsage('claude-3-5-sonnet-20241022'),
    ]
    const usage = extractUsageFromEntries(entries)

    expect(usage.length).toBe(1)
  })

  test('returns empty array for empty input', () => {
    const usage = extractUsageFromEntries([])
    expect(usage).toEqual([])
  })
})

describe('aggregateUsageStats', () => {
  test('aggregates stats from multiple entries', () => {
    const entries = [
      createAssistantEntry('claude-3-5-sonnet-20241022', 1000, 500),
      createAssistantEntry('claude-3-5-sonnet-20241022', 2000, 1000),
    ]
    const stats = aggregateUsageStats(entries)

    expect(stats.totalInputTokens).toBe(3000)
    expect(stats.totalOutputTokens).toBe(1500)
    expect(stats.totalTokens).toBe(4500)
    expect(stats.apiCalls).toBe(2)
  })

  test('calculates total cost correctly', () => {
    const entries = [
      createAssistantEntry('claude-3-5-sonnet-20241022', 1000, 500),
    ]
    const stats = aggregateUsageStats(entries)

    // (1000/1M * 3) + (500/1M * 15) = 0.003 + 0.0075 = 0.0105
    expect(stats.totalCost).toBeCloseTo(0.0105, 6)
  })

  test('provides model breakdown', () => {
    const entries = [
      createAssistantEntry('claude-3-5-sonnet-20241022', 1000, 500),
      createAssistantEntry('claude-3-5-sonnet-20241022', 2000, 1000),
      createAssistantEntry('claude-3-opus-20240229', 500, 200),
    ]
    const stats = aggregateUsageStats(entries)

    expect(stats.modelBreakdown.length).toBe(2)

    const sonnetStats = stats.modelBreakdown.find(m => m.model === 'claude-3-5-sonnet-20241022')
    expect(sonnetStats).toBeDefined()
    expect(sonnetStats!.inputTokens).toBe(3000)
    expect(sonnetStats!.outputTokens).toBe(1500)
    expect(sonnetStats!.calls).toBe(2)

    const opusStats = stats.modelBreakdown.find(m => m.model === 'claude-3-opus-20240229')
    expect(opusStats).toBeDefined()
    expect(opusStats!.inputTokens).toBe(500)
    expect(opusStats!.outputTokens).toBe(200)
    expect(opusStats!.calls).toBe(1)
  })

  test('returns zero stats for empty input', () => {
    const stats = aggregateUsageStats([])

    expect(stats.totalInputTokens).toBe(0)
    expect(stats.totalOutputTokens).toBe(0)
    expect(stats.totalTokens).toBe(0)
    expect(stats.totalCost).toBe(0)
    expect(stats.apiCalls).toBe(0)
    expect(stats.modelBreakdown).toEqual([])
  })
})
