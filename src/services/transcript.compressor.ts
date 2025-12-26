/**
 * Transcript Compressor Service
 *
 * Uses Claude Haiku to compress tool outputs into concise observations.
 * Achieves 10:1 to 100:1 compression ratio while preserving semantic meaning.
 */

import Anthropic from '@anthropic-ai/sdk';
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

/** Compression configuration */
export interface CompressionConfig {
  model?: string;
  maxTokens?: number;
  targetTokens?: number;
  apiKey?: string;
}

/** Default configuration */
const DEFAULT_CONFIG: Required<CompressionConfig> = {
  model: 'claude-3-5-haiku-latest',
  maxTokens: 1024,
  targetTokens: 500,
  apiKey: '',
};

/** Compression prompt template */
const COMPRESSION_PROMPT = `You are a memory compression agent. Your task is to compress tool outputs into concise observations that capture the essential information.

Rules:
1. Extract the key insight or action taken
2. Keep file paths and important identifiers
3. Target approximately {targetTokens} tokens output (achieving 10:1 compression)
4. Categorize appropriately based on what was done
5. Identify relevant file paths
6. Extract key concepts/keywords

Categories:
- decision: Architecture/tech decisions, design choices
- bugfix: Bug fixes, error corrections
- feature: New feature implementation
- refactor: Code refactoring, restructuring
- discovery: Code understanding, exploration, search results
- change: File modifications, edits

Tool: {toolName}
Input: {toolInput}
Output:
{toolOutput}

Respond in valid JSON format only:
{
  "content": "compressed observation in 1-3 sentences",
  "category": "one of: decision|bugfix|feature|refactor|discovery|change",
  "files": ["relevant/file/paths"],
  "concepts": ["key", "concepts", "keywords"]
}`;

/**
 * Transcript Compressor Service
 */
export class TranscriptCompressor {
  private config: Required<CompressionConfig>;
  private client: Anthropic | null = null;

  constructor(config: CompressionConfig = {}) {
    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
      apiKey: config.apiKey || process.env.ANTHROPIC_API_KEY || '',
    };
  }

  /**
   * Get or create Anthropic client
   */
  private getClient(): Anthropic {
    if (!this.client) {
      if (!this.config.apiKey) {
        throw new Error('Anthropic API key is required. Set ANTHROPIC_API_KEY environment variable.');
      }
      this.client = new Anthropic({ apiKey: this.config.apiKey });
    }
    return this.client;
  }

  /**
   * Estimate token count (rough approximation)
   */
  private estimateTokens(text: string): number {
    // Rough estimate: 1 token ≈ 4 characters
    return Math.ceil(text.length / 4);
  }

  /**
   * Compress a tool output into a concise observation
   */
  async compress(toolOutput: ToolOutput): Promise<CompressionResult> {
    const client = this.getClient();

    // Prepare the prompt
    const prompt = COMPRESSION_PROMPT
      .replace('{targetTokens}', this.config.targetTokens.toString())
      .replace('{toolName}', toolOutput.toolName)
      .replace('{toolInput}', JSON.stringify(toolOutput.toolInput, null, 2))
      .replace('{toolOutput}', this.truncateOutput(toolOutput.toolOutput));

    const originalTokens = this.estimateTokens(
      toolOutput.toolName + JSON.stringify(toolOutput.toolInput) + toolOutput.toolOutput
    );

    try {
      const response = await client.messages.create({
        model: this.config.model,
        max_tokens: this.config.maxTokens,
        messages: [
          {
            role: 'user',
            content: prompt,
          },
        ],
      });

      // Extract text content
      const textContent = response.content.find((c) => c.type === 'text');
      if (!textContent || textContent.type !== 'text') {
        throw new Error('No text content in response');
      }

      // Parse JSON response
      const result = this.parseResponse(textContent.text);
      const compressedTokens = this.estimateTokens(result.content);

      return {
        ...result,
        originalTokens,
        compressedTokens,
        compressionRatio: originalTokens / compressedTokens,
      };
    } catch (error) {
      logger.error('Compression failed', error);

      // Return a fallback compression
      return this.fallbackCompression(toolOutput, originalTokens);
    }
  }

  /**
   * Compress multiple tool outputs in batch
   */
  async compressBatch(outputs: ToolOutput[]): Promise<CompressionResult[]> {
    const results: CompressionResult[] = [];

    for (const output of outputs) {
      try {
        const result = await this.compress(output);
        results.push(result);
      } catch (error) {
        logger.error(`Failed to compress output for tool ${output.toolName}`, error);
        // Use fallback for failed compressions
        const originalTokens = this.estimateTokens(
          output.toolName + JSON.stringify(output.toolInput) + output.toolOutput
        );
        results.push(this.fallbackCompression(output, originalTokens));
      }
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

    // Keep beginning and end
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
    // Try to extract JSON from response
    const jsonMatch = text.match(/\{[\s\S]*\}/);
    if (!jsonMatch) {
      throw new Error('No JSON found in response');
    }

    const parsed = JSON.parse(jsonMatch[0]);

    // Validate and sanitize
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
    // Simple rule-based compression
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

    // Extract file paths from output (basic pattern matching)
    const filePattern = /(?:^|[\s'"(])([\/\w.-]+\.[a-zA-Z]{1,5})(?:[\s'"):]|$)/gm;
    const matches = output.toolOutput.matchAll(filePattern);
    for (const match of matches) {
      if (match[1] && !files.includes(match[1])) {
        files.push(match[1]);
      }
    }

    return files.slice(0, 10); // Limit to 10 files
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
   * Check if compression is available (API key configured)
   */
  isAvailable(): boolean {
    return !!this.config.apiKey;
  }

  /**
   * Get compression statistics
   */
  getStats(): { model: string; available: boolean } {
    return {
      model: this.config.model,
      available: this.isAvailable(),
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
