import type { AggregatedSession } from './session.types.js';
import {
  buildSessionDigestInput,
  markSessionDigestError,
} from './session-digest.service.js';
import { sessionDigestRepository } from '../infrastructure/database/repositories/session-digest.repository.js';
import {
  config as appConfig,
  type SessionDigestSummarizerConfig,
} from '../lib/config.js';

export type LocalSummarizerProvider = Exclude<
  SessionDigestSummarizerConfig['provider'],
  'disabled'
>;

export interface LocalSummarizerConfig {
  provider: LocalSummarizerProvider;
  baseUrl: string;
  model: string;
  timeoutMs: number;
  maxInputChars: number;
  maxOutputTokens: number;
}

export interface SummarizerResult {
  summary: string;
  nextActions: string[];
  blockers: string[];
  waitingForHuman: boolean;
}

type FetchLike = typeof fetch;

const MAX_SUMMARY_LENGTH = 500;
const MAX_ITEMS = 5;
const MAX_ITEM_LENGTH = 200;
export const LOCAL_SUMMARIZER_DISABLED_ERROR = 'Local summarizer is disabled';

export function getLocalSummarizerConfig(
  cfg: SessionDigestSummarizerConfig = appConfig.get().sessionDigest.summarizer
): LocalSummarizerConfig | null {
  if (cfg.provider === 'disabled') return null;
  if (
    cfg.provider !== 'ollama' &&
    cfg.provider !== 'lm_studio' &&
    cfg.provider !== 'openai_compatible_local'
  ) {
    throw new Error('Unsupported local summarizer provider');
  }

  const baseUrl = cfg.baseUrl.trim();
  const model = cfg.model.trim();
  if (!baseUrl) throw new Error('Local summarizer baseUrl is required');
  if (!model) throw new Error('Local summarizer model is required');
  assertLoopbackEndpoint(baseUrl);

  return {
    provider: cfg.provider,
    baseUrl,
    model,
    timeoutMs: cfg.timeoutMs,
    maxInputChars: cfg.maxInputChars,
    maxOutputTokens: cfg.maxOutputTokens,
  };
}

export async function generateLocalModelSessionDigest(
  session: AggregatedSession,
  options: {
    config?: LocalSummarizerConfig | null;
    fetchImpl?: FetchLike;
  } = {}
) {
  const config = options.config ?? getLocalSummarizerConfig();
  if (!config) {
    throw new Error(LOCAL_SUMMARIZER_DISABLED_ERROR);
  }

  try {
    const result = await summarizeWithLocalProvider(
      buildSessionDigestInput(session),
      config,
      options.fetchImpl ?? fetch
    );
    return sessionDigestRepository.upsert({
      sessionId: session.sessionId,
      summary: result.summary,
      nextActions: result.nextActions,
      blockers: result.blockers,
      waitingForHuman: result.waitingForHuman,
      source: 'local_model',
      status: 'fresh',
      sourceUpdatedAt: session.updatedAt,
      provider: config.provider,
    });
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    markSessionDigestError({
      session,
      source: 'local_model',
      provider: config.provider,
      errorMessage,
    });
    throw error;
  }
}

export async function summarizeWithLocalProvider(
  input: unknown,
  config: LocalSummarizerConfig,
  fetchImpl: FetchLike = fetch
): Promise<SummarizerResult> {
  assertLoopbackEndpoint(config.baseUrl);
  const endpoint = buildProviderUrl(config);
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), config.timeoutMs);
  let response: Response;
  try {
    response = await fetchImpl(endpoint, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(buildRequestBody(input, config)),
      signal: controller.signal,
    });
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      throw new Error(`Local summarizer timed out after ${config.timeoutMs}ms`);
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }

  if (!response.ok) {
    throw new Error(`Local summarizer failed with HTTP ${response.status}`);
  }

  const json = await response.json();
  const content = extractContent(json, config.provider);
  return validateSummarizerResult(parseJsonObject(content));
}

function buildRequestBody(input: unknown, config: LocalSummarizerConfig): Record<string, unknown> {
  const inputJson = JSON.stringify(input);
  const prompt = [
    'Return only one strict JSON object with keys summary, nextActions, blockers, waitingForHuman.',
    'Use only the provided local session facts. Do not invent progress, blockers, or next actions.',
    truncateText(inputJson, config.maxInputChars),
  ].join('\n\n');

  return {
    model: config.model,
    temperature: 0,
    max_tokens: config.maxOutputTokens,
    messages: [
      {
        role: 'system',
        content: 'You summarize local agent session state for Keepline. Output valid JSON only.',
      },
      { role: 'user', content: prompt },
    ],
  };
}

function buildProviderUrl(config: LocalSummarizerConfig): string {
  const url = new URL(config.baseUrl);
  const normalizedPath = url.pathname.replace(/\/+$/, '');
  if (normalizedPath.endsWith('/chat/completions')) {
    return url.toString();
  }
  if (!normalizedPath || normalizedPath === '/') {
    url.pathname = '/v1/chat/completions';
  } else if (normalizedPath.endsWith('/v1')) {
    url.pathname = `${normalizedPath}/chat/completions`;
  } else {
    url.pathname = `${normalizedPath}/chat/completions`;
  }
  return url.toString();
}

function extractContent(payload: unknown, _provider: LocalSummarizerProvider): string {
  if (!payload || typeof payload !== 'object') {
    throw new Error('Local summarizer returned invalid JSON');
  }

  const choices = (payload as {
    choices?: Array<{ message?: { content?: unknown } }>;
  }).choices;
  const content = choices?.[0]?.message?.content;
  if (typeof content === 'string') return content;

  const message = (payload as { message?: { content?: unknown } }).message;
  if (typeof message?.content === 'string') return message.content;

  throw new Error('Local summarizer response did not include message content');
}

function parseJsonObject(content: string): Record<string, unknown> {
  const trimmed = content.trim();
  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    throw new Error('Local summarizer response was not valid JSON');
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('Local summarizer response must be a JSON object');
  }
  return parsed as Record<string, unknown>;
}

function validateSummarizerResult(payload: Record<string, unknown>): SummarizerResult {
  const allowedKeys = new Set(['summary', 'nextActions', 'blockers', 'waitingForHuman']);
  const extraKey = Object.keys(payload).find((key) => !allowedKeys.has(key));
  if (extraKey) {
    throw new Error(`Local summarizer returned unexpected key: ${extraKey}`);
  }

  if (typeof payload.summary !== 'string' || !payload.summary.trim()) {
    throw new Error('Local summarizer summary must be a string');
  }
  if (payload.summary.length > MAX_SUMMARY_LENGTH) {
    throw new Error(`Local summarizer summary must be ${MAX_SUMMARY_LENGTH} characters or less`);
  }
  if (!Array.isArray(payload.nextActions)) {
    throw new Error('Local summarizer nextActions must be a string array');
  }
  if (!Array.isArray(payload.blockers)) {
    throw new Error('Local summarizer blockers must be a string array');
  }
  const nextActions = validateStringArray('nextActions', payload.nextActions);
  const blockers = validateStringArray('blockers', payload.blockers);
  if (typeof payload.waitingForHuman !== 'boolean') {
    throw new Error('Local summarizer waitingForHuman must be a boolean');
  }

  return {
    summary: payload.summary.trim(),
    nextActions,
    blockers,
    waitingForHuman: payload.waitingForHuman,
  };
}

function validateStringArray(field: string, values: unknown[]): string[] {
  if (values.length > MAX_ITEMS) {
    throw new Error(`Local summarizer ${field} must include ${MAX_ITEMS} items or fewer`);
  }

  return values.map((item) => {
    if (typeof item !== 'string' || !item.trim()) {
      throw new Error(`Local summarizer ${field} must be a string array`);
    }
    if (item.length > MAX_ITEM_LENGTH) {
      throw new Error(
        `Local summarizer ${field} items must be ${MAX_ITEM_LENGTH} characters or less`
      );
    }
    return item.trim();
  });
}

function assertLoopbackEndpoint(endpoint: string): void {
  let url: URL;
  try {
    url = new URL(endpoint);
  } catch {
    throw new Error('Local summarizer endpoint must be a valid URL');
  }

  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new Error('Local summarizer endpoint must use http or https');
  }

  const host = url.hostname.toLowerCase();
  const loopbackHosts = new Set(['localhost', '127.0.0.1', '::1']);
  if (!loopbackHosts.has(host)) {
    throw new Error('Local summarizer endpoint must be loopback');
  }
}

function truncateText(value: string, maxLength: number): string {
  return value.length <= maxLength ? value : value.slice(0, maxLength);
}
