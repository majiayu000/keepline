/**
 * Transcript Compressor Service
 *
 * Uses Claude Agent SDK (preferred) or Anthropic API to compress tool outputs
 * into concise observations. Achieves 10:1 to 100:1 compression ratio while
 * preserving semantic meaning.
 *
 * Agent SDK mode: No API key required (uses Claude Code's authentication)
 * API mode: Requires ANTHROPIC_API_KEY environment variable
 */

import { logger } from '../lib/logger.js';
import type { ObservationCategory, Observation } from '../infrastructure/vector/types.js';

/** Compression result */
export interface CompressionResult {
  content: string;
  category: ObservationCategory;
  files: string[];
  concepts: string[];
  originalTokens: number;
  compressedTokens: number;
  compressionRatio: number;
}

/** Tool output to compress */
export interface ToolOutput {
  toolName: string;
  toolInput: Record<string, unknown>;
  toolOutput: string;
  sessionId: string;
}

/** Compression mode */
export type CompressionMode = 'agent-sdk' | 'api' | 'fallback';

/** Compression configuration */
export interface CompressionConfig {
  mode?: CompressionMode;
  model?: string;
  maxTokens?: number;
  targetTokens?: number;
  apiKey?: string;
}

/** Default configuration */
const DEFAULT_CONFIG: Required<CompressionConfig> = {
  mode: 'agent-sdk', // Prefer Agent SDK (no API key needed)
  model: 'haiku',
  maxTokens: 1024,
  targetTokens: 500,
  apiKey: '',
};

/** Compression prompt template */
const COMPRESSION_PROMPT = `You are a memory compression agent. Compress this tool output into a concise observation.

Rules:
1. Extract the key insight or action taken
2. Keep file paths and important identifiers
3. Target approximately {targetTokens} tokens (10:1 compression)
4. Output valid JSON only

Categories: decision|bugfix|feature|refactor|discovery|change

Tool: {toolName}
Input: {toolInput}
Output:
{toolOutput}

Respond with ONLY valid JSON:
{"content":"1-3 sentence summary","category":"category","files":["paths"],"concepts":["keywords"]}`;

/**
 * Transcript Compressor Service
 */
export class TranscriptCompressor {
  private config: Required<CompressionConfig>;
  private sdkAvailable: boolean | null = null;
  private anthropicClient: any = null;

  constructor(config: CompressionConfig = {}) {
    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
      apiKey: config.apiKey || process.env.ANTHROPIC_API_KEY || '',
    };
  }

  /**
   * Check if Agent SDK is available
   */
  private async checkSdkAvailable(): Promise<boolean> {
    if (this.sdkAvailable !== null) return this.sdkAvailable;

    try {
      // Try to import the SDK
      const sdk = await import('@anthropic-ai/claude-agent-sdk');
      this.sdkAvailable = typeof sdk.query === 'function';
      logger.info('Claude Agent SDK available for compression');
    } catch {
      this.sdkAvailable = false;
      logger.info('Claude Agent SDK not available, will use API or fallback');
    }

    return this.sdkAvailable;
  }

  /**
   * Get Anthropic client for API mode
   */
  private async getAnthropicClient(): Promise<any> {
    if (this.anthropicClient) return this.anthropicClient;

    if (!this.config.apiKey) {
      throw new Error('Anthropic API key required for API mode');
    }

    const Anthropic = (await import('@anthropic-ai/sdk')).default;
    this.anthropicClient = new Anthropic({ apiKey: this.config.apiKey });
    return this.anthropicClient;
  }

  /**
   * Estimate token count (rough approximation)
   */
  private estimateTokens(text: string): number {
    return Math.ceil(text.length / 4);
  }

  /**
   * Compress using Claude Agent SDK (no API key needed)
   */
  private async compressWithAgentSdk(toolOutput: ToolOutput): Promise<CompressionResult | null> {
    try {
      const { query } = await import('@anthropic-ai/claude-agent-sdk');

      const prompt = this.buildPrompt(toolOutput);
      const originalTokens = this.estimateTokens(
        toolOutput.toolName + JSON.stringify(toolOutput.toolInput) + toolOutput.toolOutput
      );

      let resultText = '';

      // Use Agent SDK query
      for await (const message of query({
        prompt,
        options: {
          model: this.config.model === 'haiku' ? 'claude-3-5-haiku-latest' : this.config.model,
          maxTurns: 1,
          permissionMode: 'bypassPermissions',
          allowDangerouslySkipPermissions: true,
          tools: [], // No tools needed for compression
        },
      })) {
        if (message.type === 'result' && 'result' in message) {
          resultText = message.result;
        }
      }

      if (!resultText) {
        return null;
      }

      const parsed = this.parseResponse(resultText);
      const compressedTokens = this.estimateTokens(parsed.content);

      return {
        ...parsed,
        originalTokens,
        compressedTokens,
        compressionRatio: originalTokens / compressedTokens,
      };
    } catch (error) {
      logger.debug('Agent SDK compression failed', error);
      return null;
    }
  }

  /**
   * Compress using Anthropic API (requires API key)
   */
  private async compressWithApi(toolOutput: ToolOutput): Promise<CompressionResult | null> {
    if (!this.config.apiKey) {
      return null;
    }

    try {
      const client = await this.getAnthropicClient();
      const prompt = this.buildPrompt(toolOutput);
      const originalTokens = this.estimateTokens(
        toolOutput.toolName + JSON.stringify(toolOutput.toolInput) + toolOutput.toolOutput
      );

      const response = await client.messages.create({
        model: this.config.model === 'haiku' ? 'claude-3-5-haiku-latest' : this.config.model,
        max_tokens: this.config.maxTokens,
        messages: [{ role: 'user', content: prompt }],
      });

      const textContent = response.content.find((c: any) => c.type === 'text');
      if (!textContent || textContent.type !== 'text') {
        return null;
      }

      const parsed = this.parseResponse(textContent.text);
      const compressedTokens = this.estimateTokens(parsed.content);

      return {
        ...parsed,
        originalTokens,
        compressedTokens,
        compressionRatio: originalTokens / compressedTokens,
      };
    } catch (error) {
      logger.debug('API compression failed', error);
      return null;
    }
  }

  /**
   * Compress a tool output into a concise observation
   */
  async compress(toolOutput: ToolOutput): Promise<CompressionResult> {
    const originalTokens = this.estimateTokens(
      toolOutput.toolName + JSON.stringify(toolOutput.toolInput) + toolOutput.toolOutput
    );

    // Try Agent SDK first (preferred, no API key needed)
    if (this.config.mode === 'agent-sdk' || this.config.mode === 'api') {
      const sdkAvailable = await this.checkSdkAvailable();

      if (sdkAvailable) {
        const result = await this.compressWithAgentSdk(toolOutput);
        if (result) {
          logger.debug('Compressed with Agent SDK');
          return result;
        }
      }

      // Fall back to API if SDK fails
      if (this.config.apiKey) {
        const result = await this.compressWithApi(toolOutput);
        if (result) {
          logger.debug('Compressed with Anthropic API');
          return result;
        }
      }
    }

    // Use fallback compression
    logger.debug('Using fallback compression');
    return this.fallbackCompression(toolOutput, originalTokens);
  }

  /**
   * Build the compression prompt
   */
  private buildPrompt(toolOutput: ToolOutput): string {
    return COMPRESSION_PROMPT
      .replace('{targetTokens}', this.config.targetTokens.toString())
      .replace('{toolName}', toolOutput.toolName)
      .replace('{toolInput}', JSON.stringify(toolOutput.toolInput, null, 2))
      .replace('{toolOutput}', this.truncateOutput(toolOutput.toolOutput));
  }

  /**
   * Compress multiple tool outputs in batch
   */
  async compressBatch(outputs: ToolOutput[]): Promise<CompressionResult[]> {
    const results: CompressionResult[] = [];

    for (const output of outputs) {
      const result = await this.compress(output);
      results.push(result);
    }

    return results;
  }

  /**
   * Create an observation from compression result
   */
  createObservation(
    result: CompressionResult,
    sessionId: string,
    id?: string
  ): Observation {
    return {
      id: id || crypto.randomUUID(),
      sessionId,
      content: result.content,
      category: result.category,
      files: result.files,
      concepts: result.concepts,
      timestamp: new Date(),
      tokenCount: result.compressedTokens,
      compressed: true,
    };
  }

  /**
   * Truncate output to avoid exceeding context limits
   */
  private truncateOutput(output: string, maxChars: number = 10000): string {
    if (output.length <= maxChars) return output;

    const halfMax = Math.floor(maxChars / 2);
    return (
      output.slice(0, halfMax) +
      '\n\n... [truncated] ...\n\n' +
      output.slice(-halfMax)
    );
  }

  /**
   * Parse the JSON response from Claude
   */
  private parseResponse(text: string): Omit<CompressionResult, 'originalTokens' | 'compressedTokens' | 'compressionRatio'> {
    const jsonMatch = text.match(/\{[\s\S]*\}/);
    if (!jsonMatch) {
      throw new Error('No JSON found in response');
    }

    const parsed = JSON.parse(jsonMatch[0]);

    const validCategories: ObservationCategory[] = [
      'decision', 'bugfix', 'feature', 'refactor', 'discovery', 'change'
    ];

    const category = validCategories.includes(parsed.category)
      ? parsed.category
      : 'discovery';

    return {
      content: typeof parsed.content === 'string' ? parsed.content : 'Unknown observation',
      category,
      files: Array.isArray(parsed.files) ? parsed.files.filter((f: unknown) => typeof f === 'string') : [],
      concepts: Array.isArray(parsed.concepts) ? parsed.concepts.filter((c: unknown) => typeof c === 'string') : [],
    };
  }

  /**
   * Fallback compression when AI compression fails
   */
  private fallbackCompression(output: ToolOutput, originalTokens: number): CompressionResult {
    const content = this.createFallbackContent(output);
    const files = this.extractFiles(output);
    const category = this.inferCategory(output.toolName);
    const compressedTokens = this.estimateTokens(content);

    return {
      content,
      category,
      files,
      concepts: [output.toolName],
      originalTokens,
      compressedTokens,
      compressionRatio: originalTokens / compressedTokens,
    };
  }

  /**
   * Create fallback content from tool output
   */
  private createFallbackContent(output: ToolOutput): string {
    const toolName = output.toolName;
    const input = output.toolInput;

    switch (toolName) {
      case 'Read':
        return `Read file: ${input.file_path || 'unknown'}`;
      case 'Write':
        return `Wrote file: ${input.file_path || 'unknown'}`;
      case 'Edit':
        return `Edited file: ${input.file_path || 'unknown'}`;
      case 'Bash':
        return `Executed command: ${(input.command as string)?.slice(0, 100) || 'unknown'}`;
      case 'Glob':
        return `Searched for files: ${input.pattern || 'unknown'}`;
      case 'Grep':
        return `Searched for pattern: ${input.pattern || 'unknown'}`;
      default:
        return `Used tool ${toolName}`;
    }
  }

  /**
   * Extract file paths from tool output
   */
  private extractFiles(output: ToolOutput): string[] {
    const files: string[] = [];
    const input = output.toolInput;

    if (typeof input.file_path === 'string') {
      files.push(input.file_path);
    }
    if (typeof input.path === 'string') {
      files.push(input.path);
    }

    const filePattern = /(?:^|[\s'"(])([\/\w.-]+\.[a-zA-Z]{1,5})(?:[\s'"):]|$)/gm;
    const matches = output.toolOutput.matchAll(filePattern);
    for (const match of matches) {
      if (match[1] && !files.includes(match[1])) {
        files.push(match[1]);
      }
    }

    return files.slice(0, 10);
  }

  /**
   * Infer category from tool name
   */
  private inferCategory(toolName: string): ObservationCategory {
    switch (toolName) {
      case 'Read':
      case 'Glob':
      case 'Grep':
        return 'discovery';
      case 'Write':
        return 'feature';
      case 'Edit':
        return 'change';
      case 'Bash':
        return 'change';
      default:
        return 'discovery';
    }
  }

  /**
   * Check if AI compression is available
   */
  async isAvailable(): Promise<boolean> {
    const sdkAvailable = await this.checkSdkAvailable();
    return sdkAvailable || !!this.config.apiKey;
  }

  /**
   * Get current compression mode
   */
  async getCurrentMode(): Promise<CompressionMode> {
    const sdkAvailable = await this.checkSdkAvailable();
    if (sdkAvailable) return 'agent-sdk';
    if (this.config.apiKey) return 'api';
    return 'fallback';
  }

  /**
   * Get compression statistics
   */
  async getStats(): Promise<{ mode: CompressionMode; model: string; available: boolean }> {
    const mode = await this.getCurrentMode();
    return {
      mode,
      model: this.config.model,
      available: mode !== 'fallback',
    };
  }
}

/** Singleton instance */
let compressorInstance: TranscriptCompressor | null = null;

/**
 * Get the transcript compressor instance
 */
export function getTranscriptCompressor(config?: CompressionConfig): TranscriptCompressor {
  if (!compressorInstance) {
    compressorInstance = new TranscriptCompressor(config);
  }
  return compressorInstance;
}

/**
 * Reset the compressor instance (useful for testing)
 */
export function resetTranscriptCompressor(): void {
  compressorInstance = null;
}
