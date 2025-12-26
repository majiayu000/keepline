/**
 * Endless Mode - Dual Layer Memory Architecture
 *
 * Implements the "Endless Mode" pattern inspired by claude-mem:
 * - Working Memory: Recent observations kept in context
 * - Archive Memory: Compressed observations stored in LanceDB
 *
 * Automatically manages context window by monitoring token usage
 * and compressing/archiving older content when approaching limits.
 */

import { logger } from '../lib/logger.js';
import { on } from '../lib/events.js';
import { getVectorStore } from '../infrastructure/vector/lancedb.adapter.js';
import { getEmbeddingService } from '../infrastructure/vector/embedding.service.js';
import { getTranscriptCompressor } from './transcript.compressor.js';
import type { Observation } from '../infrastructure/vector/types.js';
import type { ToolOutput } from './transcript.compressor.js';

/** Endless mode configuration */
export interface EndlessModeConfig {
  /** Maximum tokens for working memory before compression */
  maxWorkingMemoryTokens: number;
  /** Target tokens after compression */
  targetWorkingMemoryTokens: number;
  /** Minimum observations to keep in working memory */
  minWorkingMemoryItems: number;
  /** Enable auto-compression when threshold reached */
  autoCompressEnabled: boolean;
  /** Token count threshold that triggers compression (percentage of max) */
  compressionTriggerThreshold: number;
}

/** Working memory item */
export interface WorkingMemoryItem {
  id: string;
  sessionId: string;
  content: string;
  timestamp: Date;
  tokenCount: number;
  compressed: boolean;
  archived: boolean;
}

/** Endless mode statistics */
export interface EndlessModeStats {
  workingMemoryItems: number;
  workingMemoryTokens: number;
  archivedItems: number;
  compressionEvents: number;
  lastCompressionAt: Date | null;
  isCompressing: boolean;
}

/** Default configuration */
const DEFAULT_CONFIG: EndlessModeConfig = {
  maxWorkingMemoryTokens: 50000, // ~50k tokens working memory
  targetWorkingMemoryTokens: 25000, // Compress to ~25k tokens
  minWorkingMemoryItems: 5, // Keep at least 5 recent items
  autoCompressEnabled: true,
  compressionTriggerThreshold: 0.8, // Trigger at 80% capacity
};

/**
 * Endless Mode Service
 *
 * Manages the dual-layer memory architecture for unlimited context.
 */
export class EndlessModeService {
  private config: EndlessModeConfig;
  private workingMemory: Map<string, WorkingMemoryItem[]> = new Map();
  private stats: EndlessModeStats = {
    workingMemoryItems: 0,
    workingMemoryTokens: 0,
    archivedItems: 0,
    compressionEvents: 0,
    lastCompressionAt: null,
    isCompressing: false,
  };
  private initialized = false;

  constructor(config: Partial<EndlessModeConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Initialize the endless mode service
   */
  async initialize(): Promise<void> {
    if (this.initialized) return;

    // Subscribe to tool events for automatic memory updates
    on('tool:post', (payload) => {
      if (payload.output) {
        this.addToWorkingMemory(payload.sessionId, {
          toolName: payload.tool,
          toolInput: payload.input,
          toolOutput: payload.output,
          sessionId: payload.sessionId,
        }).catch((error) => {
          logger.error('Failed to add to working memory', error);
        });
      }
    });

    // Subscribe to session end for cleanup
    on('session:end', (payload) => {
      this.archiveSession(payload.sessionId).catch((error) => {
        logger.error('Failed to archive session', error);
      });
    });

    this.initialized = true;
    logger.info('Endless mode initialized');
  }

  /**
   * Add tool output to working memory
   */
  async addToWorkingMemory(sessionId: string, toolOutput: ToolOutput): Promise<void> {
    const items = this.workingMemory.get(sessionId) || [];

    // Estimate token count
    const tokenCount = this.estimateTokens(
      toolOutput.toolName + JSON.stringify(toolOutput.toolInput) + toolOutput.toolOutput
    );

    const item: WorkingMemoryItem = {
      id: crypto.randomUUID(),
      sessionId,
      content: this.formatToolOutput(toolOutput),
      timestamp: new Date(),
      tokenCount,
      compressed: false,
      archived: false,
    };

    items.push(item);
    this.workingMemory.set(sessionId, items);

    // Update stats
    this.updateStats();

    // Check if compression needed
    const sessionTokens = this.getSessionTokenCount(sessionId);
    if (this.shouldTriggerCompression(sessionTokens)) {
      await this.compressWorkingMemory(sessionId);
    }

    logger.debug(`Added to working memory: ${item.id} (${tokenCount} tokens)`);
  }

  /**
   * Get working memory for a session
   */
  getWorkingMemory(sessionId: string): WorkingMemoryItem[] {
    return this.workingMemory.get(sessionId) || [];
  }

  /**
   * Get working memory as formatted context
   */
  getWorkingMemoryContext(sessionId: string): string {
    const items = this.getWorkingMemory(sessionId);
    if (items.length === 0) return '';

    const lines: string[] = [
      '## Session Working Memory',
      '',
    ];

    for (const item of items) {
      const time = item.timestamp.toISOString().slice(11, 19);
      const status = item.compressed ? '[compressed]' : '';
      lines.push(`[${time}] ${status} ${item.content}`);
      lines.push('');
    }

    return lines.join('\n');
  }

  /**
   * Check if compression should be triggered
   */
  private shouldTriggerCompression(currentTokens: number): boolean {
    if (!this.config.autoCompressEnabled) return false;
    if (this.stats.isCompressing) return false;

    const threshold = this.config.maxWorkingMemoryTokens * this.config.compressionTriggerThreshold;
    return currentTokens >= threshold;
  }

  /**
   * Compress working memory for a session
   */
  async compressWorkingMemory(sessionId: string): Promise<void> {
    if (this.stats.isCompressing) {
      logger.debug('Compression already in progress');
      return;
    }

    this.stats.isCompressing = true;
    const items = this.workingMemory.get(sessionId) || [];

    if (items.length <= this.config.minWorkingMemoryItems) {
      this.stats.isCompressing = false;
      return;
    }

    logger.info(`Starting working memory compression for session ${sessionId}`);

    try {
      const compressor = getTranscriptCompressor();
      const vectorStore = getVectorStore();
      const embeddingService = getEmbeddingService();

      await vectorStore.initialize();

      // Determine how many items to compress (keep recent ones)
      const itemsToKeep = this.config.minWorkingMemoryItems;
      const itemsToCompress = items.slice(0, -itemsToKeep);
      const itemsToRetain = items.slice(-itemsToKeep);

      // Compress and archive older items
      let archivedCount = 0;
      for (const item of itemsToCompress) {
        if (item.archived) continue;

        try {
          // If not already compressed, compress it
          if (!item.compressed) {
            const compressionResult = await compressor.compress({
              toolName: 'WorkingMemory',
              toolInput: {},
              toolOutput: item.content,
              sessionId: item.sessionId,
            });

            // Create observation from compression result
            const observation: Observation = {
              id: item.id,
              sessionId: item.sessionId,
              content: compressionResult.content,
              category: compressionResult.category,
              files: compressionResult.files,
              concepts: compressionResult.concepts,
              timestamp: item.timestamp,
              tokenCount: compressionResult.compressedTokens,
              compressed: true,
            };

            // Generate embedding and store
            const vector = await embeddingService.embed(observation.content);
            await vectorStore.insert(observation, vector);

            archivedCount++;
          }

          item.archived = true;
        } catch (error) {
          logger.error(`Failed to compress item ${item.id}`, error);
        }
      }

      // Update working memory to only keep recent items
      this.workingMemory.set(sessionId, itemsToRetain);

      // Update stats
      this.stats.archivedItems += archivedCount;
      this.stats.compressionEvents++;
      this.stats.lastCompressionAt = new Date();
      this.updateStats();

      logger.info(
        `Compressed ${archivedCount} items, retained ${itemsToRetain.length} in working memory`
      );
    } finally {
      this.stats.isCompressing = false;
    }
  }

  /**
   * Archive all working memory for a session (on session end)
   */
  async archiveSession(sessionId: string): Promise<void> {
    const items = this.workingMemory.get(sessionId);
    if (!items || items.length === 0) return;

    logger.info(`Archiving session ${sessionId} with ${items.length} items`);

    try {
      // Force compression of all remaining items
      const compressor = getTranscriptCompressor();
      const vectorStore = getVectorStore();
      const embeddingService = getEmbeddingService();

      await vectorStore.initialize();

      for (const item of items) {
        if (item.archived) continue;

        try {
          const compressionResult = await compressor.compress({
            toolName: 'SessionEnd',
            toolInput: {},
            toolOutput: item.content,
            sessionId: item.sessionId,
          });

          const observation: Observation = {
            id: item.id,
            sessionId: item.sessionId,
            content: compressionResult.content,
            category: compressionResult.category,
            files: compressionResult.files,
            concepts: compressionResult.concepts,
            timestamp: item.timestamp,
            tokenCount: compressionResult.compressedTokens,
            compressed: true,
          };

          const vector = await embeddingService.embed(observation.content);
          await vectorStore.insert(observation, vector);

          this.stats.archivedItems++;
        } catch (error) {
          logger.error(`Failed to archive item ${item.id}`, error);
        }
      }

      // Clear working memory for this session
      this.workingMemory.delete(sessionId);
      this.updateStats();

      logger.info(`Session ${sessionId} archived`);
    } catch (error) {
      logger.error(`Failed to archive session ${sessionId}`, error);
    }
  }

  /**
   * Get token count for a session's working memory
   */
  getSessionTokenCount(sessionId: string): number {
    const items = this.workingMemory.get(sessionId) || [];
    return items.reduce((sum, item) => sum + item.tokenCount, 0);
  }

  /**
   * Get total token count across all sessions
   */
  getTotalTokenCount(): number {
    let total = 0;
    for (const items of this.workingMemory.values()) {
      total += items.reduce((sum, item) => sum + item.tokenCount, 0);
    }
    return total;
  }

  /**
   * Get endless mode statistics
   */
  getStats(): EndlessModeStats {
    return { ...this.stats };
  }

  /**
   * Update statistics
   */
  private updateStats(): void {
    let totalItems = 0;
    let totalTokens = 0;

    for (const items of this.workingMemory.values()) {
      totalItems += items.length;
      totalTokens += items.reduce((sum, item) => sum + item.tokenCount, 0);
    }

    this.stats.workingMemoryItems = totalItems;
    this.stats.workingMemoryTokens = totalTokens;
  }

  /**
   * Estimate token count for text
   */
  private estimateTokens(text: string): number {
    return Math.ceil(text.length / 4);
  }

  /**
   * Format tool output for working memory
   */
  private formatToolOutput(output: ToolOutput): string {
    const inputStr = JSON.stringify(output.toolInput);
    const truncatedOutput =
      output.toolOutput.length > 500
        ? output.toolOutput.slice(0, 500) + '...'
        : output.toolOutput;

    return `[${output.toolName}] ${inputStr.slice(0, 100)}: ${truncatedOutput}`;
  }

  /**
   * Clear all working memory (for testing)
   */
  clear(): void {
    this.workingMemory.clear();
    this.stats = {
      workingMemoryItems: 0,
      workingMemoryTokens: 0,
      archivedItems: 0,
      compressionEvents: 0,
      lastCompressionAt: null,
      isCompressing: false,
    };
  }
}

/** Singleton instance */
let endlessModeInstance: EndlessModeService | null = null;

/**
 * Get the endless mode service instance
 */
export function getEndlessModeService(
  config?: Partial<EndlessModeConfig>
): EndlessModeService {
  if (!endlessModeInstance) {
    endlessModeInstance = new EndlessModeService(config);
  }
  return endlessModeInstance;
}

/**
 * Initialize endless mode
 */
export async function initializeEndlessMode(
  config?: Partial<EndlessModeConfig>
): Promise<void> {
  const service = getEndlessModeService(config);
  await service.initialize();
}

/**
 * Get endless mode stats
 */
export function getEndlessModeStats(): EndlessModeStats {
  const service = getEndlessModeService();
  return service.getStats();
}

/**
 * Reset endless mode (for testing)
 */
export function resetEndlessMode(): void {
  if (endlessModeInstance) {
    endlessModeInstance.clear();
    endlessModeInstance = null;
  }
}
