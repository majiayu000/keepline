import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { sessionRepository } from '../infrastructure/database/repositories/session.repository.js';
import { sessionDigestRepository } from '../infrastructure/database/repositories/session-digest.repository.js';
import type { AggregatedSession } from '../services/session.types.js';
import {
  generateDeterministicSessionDigest,
} from '../services/session-digest.service.js';
import {
  getLocalSummarizerConfig,
  generateLocalModelSessionDigest,
  LOCAL_SUMMARIZER_DISABLED_ERROR,
  summarizeWithLocalProvider,
  type LocalSummarizerConfig,
} from '../services/orchestrator.summarizer.js';

const LOCAL_CONFIG: LocalSummarizerConfig = {
  provider: 'ollama',
  baseUrl: 'http://127.0.0.1:11434/v1',
  model: 'llama3.2',
  timeoutMs: 1000,
  maxInputChars: 1000,
  maxOutputTokens: 128,
};

function aggregatedSession(sessionId: string): AggregatedSession {
  const session = sessionRepository.findBySessionId(sessionId);
  if (!session) throw new Error(`Missing test session ${sessionId}`);
  return {
    ...session,
    processRunning: false,
  };
}

describe('orchestrator local summarizer', () => {
  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
  });

  test('disabled config returns no provider', () => {
    const cfg = getLocalSummarizerConfig({
      provider: 'disabled',
      baseUrl: 'http://127.0.0.1:11434/v1',
      model: '',
      timeoutMs: 30000,
      maxInputChars: 12000,
      maxOutputTokens: 800,
    });

    expect(cfg).toBeNull();
  });

  test('rejects non-loopback provider endpoints', () => {
    expect(() => getLocalSummarizerConfig({
      provider: 'ollama',
      baseUrl: 'https://api.example.com/v1',
      model: 'remote-model',
      timeoutMs: 30000,
      maxInputChars: 12000,
      maxOutputTokens: 800,
    })).toThrow('loopback');
  });

  test('calls local OpenAI-compatible endpoint and validates JSON content', async () => {
    let requestUrl = '';
    let requestBody: Record<string, unknown> = {};
    const fetchImpl = (async (
      url: Parameters<typeof fetch>[0],
      init?: Parameters<typeof fetch>[1]
    ) => {
      requestUrl = String(url);
      requestBody = JSON.parse(String(init?.body)) as Record<string, unknown>;
      return new Response(JSON.stringify({
        choices: [{
          message: {
            content: JSON.stringify({
              summary: 'Model summary',
              nextActions: ['Review queue'],
              blockers: [],
              waitingForHuman: true,
            }),
          },
        }],
      }), { status: 200 });
    }) as unknown as typeof fetch;

    const result = await summarizeWithLocalProvider(
      { sessionId: 'summary-input', title: 'Summarize me' },
      LOCAL_CONFIG,
      fetchImpl
    );

    expect(requestUrl).toBe('http://127.0.0.1:11434/v1/chat/completions');
    expect(requestBody).toMatchObject({
      model: 'llama3.2',
      temperature: 0,
      max_tokens: 128,
    });
    expect(result).toEqual({
      summary: 'Model summary',
      nextActions: ['Review queue'],
      blockers: [],
      waitingForHuman: true,
    });
  });

  test('rejects model output with extra keys instead of silently accepting it', async () => {
    const fetchImpl = (async () => new Response(JSON.stringify({
      choices: [{
        message: {
          content: JSON.stringify({
            summary: 'Model summary',
            nextActions: [],
            blockers: [],
            waitingForHuman: false,
            inventedField: 'no',
          }),
        },
      }],
    }), { status: 200 })) as unknown as typeof fetch;

    await expect(summarizeWithLocalProvider(
      { sessionId: 'summary-input' },
      LOCAL_CONFIG,
      fetchImpl
    )).rejects.toThrow('unexpected key');
  });

  test('disabled local model generation does not write an error digest', async () => {
    sessionRepository.upsert({
      sessionId: 'summarizer-disabled',
      directory: '/tmp/keepline-summarizer-disabled',
      status: 'running',
      title: 'Disabled summarizer',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
    });

    await expect(generateLocalModelSessionDigest(
      aggregatedSession('summarizer-disabled'),
      { config: null }
    )).rejects.toThrow(LOCAL_SUMMARIZER_DISABLED_ERROR);

    expect(sessionDigestRepository.findBySessionId('summarizer-disabled')).toBeNull();
  });

  test('provider failure marks digest error while preserving old summary', async () => {
    sessionRepository.upsert({
      sessionId: 'summarizer-failure',
      directory: '/tmp/keepline-summarizer-failure',
      status: 'running',
      title: 'Provider failure',
      lastMessage: 'Existing deterministic content',
      lastActiveAt: new Date('2026-06-29T10:00:00.000Z'),
    });
    generateDeterministicSessionDigest(aggregatedSession('summarizer-failure'));
    const fetchImpl = (async () =>
      new Response('provider down', { status: 503 })) as unknown as typeof fetch;

    await expect(generateLocalModelSessionDigest(
      aggregatedSession('summarizer-failure'),
      { config: LOCAL_CONFIG, fetchImpl }
    )).rejects.toThrow('HTTP 503');

    const digest = sessionDigestRepository.findBySessionId('summarizer-failure');
    expect(digest).toMatchObject({
      summary: 'Existing deterministic content',
      source: 'deterministic',
      status: 'error',
      provider: 'ollama',
      errorMessage: 'Local summarizer failed with HTTP 503',
    });
  });
});
