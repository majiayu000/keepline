/**
 * Async Compression Queue
 *
 * Non-blocking queue for processing tool outputs through AI compression
 * and storing in the vector database.
 */

import { logger } from '../lib/logger.js';
import { getTranscriptCompressor, type ToolOutput } from './transcript.compressor.js';
import { getVectorStore } from '../infrastructure/vector/lancedb.adapter.js';
import { getEmbeddingService } from '../infrastructure/vector/embedding.service.js';
import type { Observation } from '../infrastructure/vector/types.js';

/** Queue item */
interface QueueItem {
  id: string;
  toolOutput: ToolOutput;
  timestamp: Date;
  retries: number;
}

/** Queue configuration */
export interface CompressionQueueConfig {
  maxConcurrent?: number;
  maxRetries?: number;
  retryDelayMs?: number;
  batchSize?: number;
  flushIntervalMs?: number;
  minTokensToCompress?: number;
}

/** Queue statistics */
export interface QueueStats {
  pending: number;
  processing: number;
  completed: number;
  failed: number;
  totalCompressed: number;
  averageCompressionRatio: number;
}

/** Default configuration */
const DEFAULT_CONFIG: Required<CompressionQueueConfig> = {
  maxConcurrent: 3,
  maxRetries: 2,
  retryDelayMs: 1000,
  batchSize: 5,
  flushIntervalMs: 5000,
  minTokensToCompress: 100, // Only compress outputs with > 100 tokens
};

/**
 * Async Compression Queue
 */
export class CompressionQueue {
  private config: Required<CompressionQueueConfig>;
  private queue: QueueItem[] = [];
  private processing: Set<string> = new Set();
  private stats: QueueStats = {
    pending: 0,
    processing: 0,
    completed: 0,
    failed: 0,
    totalCompressed: 0,
    averageCompressionRatio: 0,
  };
  private compressionRatios: number[] = [];
  private flushInterval: ReturnType<typeof setInterval> | null = null;
  private isRunning = false;

  constructor(config: CompressionQueueConfig = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Start the queue processor
   */
  start(): void {
    if (this.isRunning) return;

    this.isRunning = true;

    // Start periodic flush
    this.flushInterval = setInterval(() => {
      this.processQueue().catch((error) => {
        logger.error('Queue flush failed', error);
      });
    }, this.config.flushIntervalMs);

    logger.info('Compression queue started');
  }

  /**
   * Stop the queue processor
   */
  async stop(): Promise<void> {
    if (!this.isRunning) return;

    this.isRunning = false;

    if (this.flushInterval) {
      clearInterval(this.flushInterval);
      this.flushInterval = null;
    }

    // Process remaining items
    if (this.queue.length > 0) {
      logger.info(`Processing ${this.queue.length} remaining items before shutdown`);
      await this.processQueue();
    }

    logger.info('Compression queue stopped');
  }

  /**
   * Add a tool output to the compression queue
   */
  enqueue(toolOutput: ToolOutput): string {
    const id = crypto.randomUUID();

    const item: QueueItem = {
      id,
      toolOutput,
      timestamp: new Date(),
      retries: 0,
    };

    this.queue.push(item);
    this.stats.pending++;

    logger.debug(`Enqueued compression task ${id} for tool ${toolOutput.toolName}`);

    // Trigger immediate processing if we have enough items
    if (this.queue.length >= this.config.batchSize) {
      setImmediate(() => {
        this.processQueue().catch((error) => {
          logger.error('Immediate queue processing failed', error);
        });
      });
    }

    return id;
  }

  /**
   * Process the queue
   */
  private async processQueue(): Promise<void> {
    if (this.queue.length === 0) return;
    if (this.processing.size >= this.config.maxConcurrent) return;

    const compressor = getTranscriptCompressor();
    const vectorStore = getVectorStore();
    const embeddingService = getEmbeddingService();

    // Get items to process
    const available = this.config.maxConcurrent - this.processing.size;
    const toProcess = this.queue.splice(0, Math.min(available, this.config.batchSize));

    if (toProcess.length === 0) return;

    // Mark as processing
    for (const item of toProcess) {
      this.processing.add(item.id);
      this.stats.pending--;
      this.stats.processing++;
    }

    // Process each item
    const results = await Promise.allSettled(
      toProcess.map((item) => this.processItem(item, compressor, vectorStore, embeddingService))
    );

    // Handle results
    for (let i = 0; i < results.length; i++) {
      const result = results[i];
      const item = toProcess[i];

      this.processing.delete(item.id);
      this.stats.processing--;

      if (result.status === 'fulfilled') {
        this.stats.completed++;
        if (result.value) {
          this.stats.totalCompressed++;
          this.compressionRatios.push(result.value.compressionRatio);
          this.updateAverageRatio();
        }
      } else {
        // Handle failure
        if (item.retries < this.config.maxRetries) {
          // Retry
          item.retries++;
          this.queue.push(item);
          this.stats.pending++;
          logger.warn(`Retrying compression task ${item.id}, attempt ${item.retries + 1}`);
        } else {
          this.stats.failed++;
          logger.error(`Compression task ${item.id} failed after ${item.retries + 1} attempts`, result.reason);
        }
      }
    }
  }

  /**
   * Process a single queue item
   */
  private async processItem(
    item: QueueItem,
    compressor: ReturnType<typeof getTranscriptCompressor>,
    vectorStore: ReturnType<typeof getVectorStore>,
    embeddingService: ReturnType<typeof getEmbeddingService>
  ): Promise<{ observation: Observation; compressionRatio: number } | null> {
    const { toolOutput } = item;

    // Estimate token count
    const estimatedTokens = Math.ceil(
      (toolOutput.toolName.length +
        JSON.stringify(toolOutput.toolInput).length +
        toolOutput.toolOutput.length) /
        4
    );

    // Skip small outputs
    if (estimatedTokens < this.config.minTokensToCompress) {
      logger.debug(`Skipping compression for ${toolOutput.toolName}: ${estimatedTokens} tokens < minimum`);
      return null;
    }

    // Compress
    const compressionResult = await compressor.compress(toolOutput);

    // Create observation
    const observation = compressor.createObservation(
      compressionResult,
      toolOutput.sessionId,
      item.id
    );

    // Generate embedding
    const vector = await embeddingService.embed(observation.content);

    // Initialize vector store if needed
    await vectorStore.initialize();

    // Store in vector database
    await vectorStore.insert(observation, vector);

    logger.info(
      `Compressed ${toolOutput.toolName}: ${compressionResult.originalTokens} → ${compressionResult.compressedTokens} tokens (${compressionResult.compressionRatio.toFixed(1)}x)`
    );

    return {
      observation,
      compressionRatio: compressionResult.compressionRatio,
    };
  }

  /**
   * Update average compression ratio
   */
  private updateAverageRatio(): void {
    if (this.compressionRatios.length === 0) {
      this.stats.averageCompressionRatio = 0;
      return;
    }

    // Keep only last 100 ratios
    if (this.compressionRatios.length > 100) {
      this.compressionRatios = this.compressionRatios.slice(-100);
    }

    const sum = this.compressionRatios.reduce((a, b) => a + b, 0);
    this.stats.averageCompressionRatio = sum / this.compressionRatios.length;
  }

  /**
   * Get queue statistics
   */
  getStats(): QueueStats {
    return { ...this.stats };
  }

  /**
   * Check if queue is running
   */
  isActive(): boolean {
    return this.isRunning;
  }

  /**
   * Get pending count
   */
  getPendingCount(): number {
    return this.queue.length;
  }

  /**
   * Force flush all pending items
   */
  async flush(): Promise<void> {
    while (this.queue.length > 0 || this.processing.size > 0) {
      await this.processQueue();
      // Wait for processing to complete
      if (this.processing.size > 0) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    }
  }
}

/** Singleton instance */
let queueInstance: CompressionQueue | null = null;

/**
 * Get the compression queue instance
 */
export function getCompressionQueue(config?: CompressionQueueConfig): CompressionQueue {
  if (!queueInstance) {
    queueInstance = new CompressionQueue(config);
  }
  return queueInstance;
}

/**
 * Start the compression queue
 */
export function startCompressionQueue(config?: CompressionQueueConfig): void {
  const queue = getCompressionQueue(config);
  queue.start();
}

/**
 * Stop the compression queue
 */
export async function stopCompressionQueue(): Promise<void> {
  if (queueInstance) {
    await queueInstance.stop();
  }
}

/**
 * Enqueue a tool output for compression
 */
export function enqueueCompression(toolOutput: ToolOutput): string {
  const queue = getCompressionQueue();
  return queue.enqueue(toolOutput);
}

/**
 * Reset the compression queue (for testing)
 */
export function resetCompressionQueue(): void {
  if (queueInstance) {
    queueInstance.stop();
    queueInstance = null;
  }
}
