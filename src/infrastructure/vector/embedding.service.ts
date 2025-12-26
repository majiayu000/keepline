/**
 * Embedding Service
 *
 * Generates vector embeddings for text using multiple providers.
 * Supports: OpenAI, Anthropic Voyage, and local embeddings.
 */

import { logger } from '../../lib/logger.js';

/** Embedding provider types */
export type EmbeddingProvider = 'openai' | 'voyage' | 'local';

/** Embedding configuration */
export interface EmbeddingConfig {
  provider: EmbeddingProvider;
  apiKey?: string;
  model?: string;
  dimension?: number;
  batchSize?: number;
}

/** Default configurations per provider */
const PROVIDER_DEFAULTS: Record<EmbeddingProvider, Partial<EmbeddingConfig>> = {
  openai: {
    model: 'text-embedding-ada-002',
    dimension: 1536,
    batchSize: 100,
  },
  voyage: {
    model: 'voyage-2',
    dimension: 1024,
    batchSize: 128,
  },
  local: {
    model: 'all-MiniLM-L6-v2',
    dimension: 384,
    batchSize: 32,
  },
};

/** Embedding service interface */
export interface IEmbeddingService {
  /** Generate embedding for a single text */
  embed(text: string): Promise<number[]>;

  /** Generate embeddings for multiple texts */
  embedBatch(texts: string[]): Promise<number[][]>;

  /** Get the embedding dimension */
  getDimension(): number;

  /** Get the provider name */
  getProvider(): EmbeddingProvider;
}

/**
 * OpenAI Embedding Service
 */
class OpenAIEmbeddingService implements IEmbeddingService {
  private config: EmbeddingConfig;

  constructor(config: EmbeddingConfig) {
    this.config = {
      ...PROVIDER_DEFAULTS.openai,
      ...config,
    };
  }

  async embed(text: string): Promise<number[]> {
    const [embedding] = await this.embedBatch([text]);
    return embedding;
  }

  async embedBatch(texts: string[]): Promise<number[][]> {
    if (!this.config.apiKey) {
      throw new Error('OpenAI API key is required. Set OPENAI_API_KEY environment variable.');
    }

    const results: number[][] = [];
    const batchSize = this.config.batchSize || 100;

    for (let i = 0; i < texts.length; i += batchSize) {
      const batch = texts.slice(i, i + batchSize);

      const response = await fetch('https://api.openai.com/v1/embeddings', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${this.config.apiKey}`,
        },
        body: JSON.stringify({
          model: this.config.model,
          input: batch,
        }),
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(`OpenAI embedding failed: ${error}`);
      }

      const data = (await response.json()) as {
        data: Array<{ embedding: number[]; index: number }>;
      };

      // Sort by index to maintain order
      const sortedEmbeddings = data.data
        .sort((a, b) => a.index - b.index)
        .map((item) => item.embedding);

      results.push(...sortedEmbeddings);
    }

    return results;
  }

  getDimension(): number {
    return this.config.dimension || 1536;
  }

  getProvider(): EmbeddingProvider {
    return 'openai';
  }
}

/**
 * Voyage AI Embedding Service (Anthropic's recommended embedding provider)
 */
class VoyageEmbeddingService implements IEmbeddingService {
  private config: EmbeddingConfig;

  constructor(config: EmbeddingConfig) {
    this.config = {
      ...PROVIDER_DEFAULTS.voyage,
      ...config,
    };
  }

  async embed(text: string): Promise<number[]> {
    const [embedding] = await this.embedBatch([text]);
    return embedding;
  }

  async embedBatch(texts: string[]): Promise<number[][]> {
    if (!this.config.apiKey) {
      throw new Error('Voyage API key is required. Set VOYAGE_API_KEY environment variable.');
    }

    const results: number[][] = [];
    const batchSize = this.config.batchSize || 128;

    for (let i = 0; i < texts.length; i += batchSize) {
      const batch = texts.slice(i, i + batchSize);

      const response = await fetch('https://api.voyageai.com/v1/embeddings', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${this.config.apiKey}`,
        },
        body: JSON.stringify({
          model: this.config.model,
          input: batch,
        }),
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(`Voyage embedding failed: ${error}`);
      }

      const data = (await response.json()) as {
        data: Array<{ embedding: number[]; index: number }>;
      };

      // Sort by index to maintain order
      const sortedEmbeddings = data.data
        .sort((a, b) => a.index - b.index)
        .map((item) => item.embedding);

      results.push(...sortedEmbeddings);
    }

    return results;
  }

  getDimension(): number {
    return this.config.dimension || 1024;
  }

  getProvider(): EmbeddingProvider {
    return 'voyage';
  }
}

/**
 * Local Embedding Service using simple TF-IDF based approach
 * Falls back when no API keys are available
 */
class LocalEmbeddingService implements IEmbeddingService {
  private config: EmbeddingConfig;

  constructor(config: EmbeddingConfig) {
    this.config = {
      ...PROVIDER_DEFAULTS.local,
      ...config,
    };
  }

  async embed(text: string): Promise<number[]> {
    // Simple hash-based embedding for local fallback
    // This is NOT semantic, just a deterministic vector for testing
    const dimension = this.config.dimension || 384;
    const vector = new Array(dimension).fill(0);

    // Tokenize
    const tokens = this.tokenize(text);

    // Hash each token to vector positions
    for (const token of tokens) {
      const hash = this.hashString(token);
      const positions = this.getPositions(hash, dimension, 5);

      for (const pos of positions) {
        vector[pos] += 1 / Math.sqrt(tokens.length);
      }
    }

    // Normalize
    return this.normalize(vector);
  }

  async embedBatch(texts: string[]): Promise<number[][]> {
    return Promise.all(texts.map((text) => this.embed(text)));
  }

  getDimension(): number {
    return this.config.dimension || 384;
  }

  getProvider(): EmbeddingProvider {
    return 'local';
  }

  private tokenize(text: string): string[] {
    return text
      .toLowerCase()
      .replace(/[^\w\s\u4e00-\u9fff]/g, ' ')
      .split(/\s+/)
      .filter((t) => t.length > 1);
  }

  private hashString(str: string): number {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = (hash << 5) - hash + char;
      hash = hash & hash; // Convert to 32bit integer
    }
    return Math.abs(hash);
  }

  private getPositions(hash: number, dimension: number, count: number): number[] {
    const positions: number[] = [];
    let h = hash;
    for (let i = 0; i < count; i++) {
      positions.push(h % dimension);
      h = this.hashString(h.toString() + i);
    }
    return positions;
  }

  private normalize(vector: number[]): number[] {
    const magnitude = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0));
    if (magnitude === 0) return vector;
    return vector.map((val) => val / magnitude);
  }
}

/** Embedding cache to avoid redundant API calls */
class EmbeddingCache {
  private cache: Map<string, { embedding: number[]; timestamp: number }> = new Map();
  private maxSize: number;
  private ttlMs: number;

  constructor(maxSize = 1000, ttlMs = 3600000) {
    // 1 hour TTL
    this.maxSize = maxSize;
    this.ttlMs = ttlMs;
  }

  get(text: string): number[] | null {
    const entry = this.cache.get(text);
    if (!entry) return null;

    if (Date.now() - entry.timestamp > this.ttlMs) {
      this.cache.delete(text);
      return null;
    }

    return entry.embedding;
  }

  set(text: string, embedding: number[]): void {
    // Evict oldest if at capacity
    if (this.cache.size >= this.maxSize) {
      const oldest = this.findOldest();
      if (oldest) this.cache.delete(oldest);
    }

    this.cache.set(text, { embedding, timestamp: Date.now() });
  }

  private findOldest(): string | null {
    let oldest: string | null = null;
    let oldestTime = Infinity;

    for (const [key, value] of this.cache.entries()) {
      if (value.timestamp < oldestTime) {
        oldestTime = value.timestamp;
        oldest = key;
      }
    }

    return oldest;
  }

  clear(): void {
    this.cache.clear();
  }

  size(): number {
    return this.cache.size;
  }
}

/**
 * Main Embedding Service with caching and provider management
 */
export class EmbeddingService implements IEmbeddingService {
  private provider: IEmbeddingService;
  private cache: EmbeddingCache;
  private config: EmbeddingConfig;

  constructor(config?: Partial<EmbeddingConfig>) {
    // Auto-detect provider based on available API keys
    const detectedProvider = this.detectProvider(config?.provider);

    this.config = {
      provider: detectedProvider,
      apiKey: this.getApiKey(detectedProvider),
      ...PROVIDER_DEFAULTS[detectedProvider],
      ...config,
    };

    // Initialize the appropriate provider
    this.provider = this.createProvider(this.config);
    this.cache = new EmbeddingCache();

    logger.info(`Embedding service initialized with provider: ${this.config.provider}`);
  }

  private detectProvider(preferred?: EmbeddingProvider): EmbeddingProvider {
    if (preferred) {
      const key = this.getApiKey(preferred);
      if (key || preferred === 'local') {
        return preferred;
      }
      logger.warn(`Preferred provider ${preferred} not available, detecting...`);
    }

    // Check for API keys in order of preference
    if (process.env.VOYAGE_API_KEY) return 'voyage';
    if (process.env.OPENAI_API_KEY) return 'openai';

    logger.warn('No embedding API keys found, using local fallback');
    return 'local';
  }

  private getApiKey(provider: EmbeddingProvider): string | undefined {
    switch (provider) {
      case 'openai':
        return process.env.OPENAI_API_KEY;
      case 'voyage':
        return process.env.VOYAGE_API_KEY;
      default:
        return undefined;
    }
  }

  private createProvider(config: EmbeddingConfig): IEmbeddingService {
    switch (config.provider) {
      case 'openai':
        return new OpenAIEmbeddingService(config);
      case 'voyage':
        return new VoyageEmbeddingService(config);
      case 'local':
      default:
        return new LocalEmbeddingService(config);
    }
  }

  async embed(text: string): Promise<number[]> {
    // Check cache first
    const cached = this.cache.get(text);
    if (cached) {
      logger.debug('Embedding cache hit');
      return cached;
    }

    // Generate embedding
    const embedding = await this.provider.embed(text);

    // Cache the result
    this.cache.set(text, embedding);

    return embedding;
  }

  async embedBatch(texts: string[]): Promise<number[][]> {
    const results: (number[] | null)[] = new Array(texts.length).fill(null);
    const uncachedIndices: number[] = [];
    const uncachedTexts: string[] = [];

    // Check cache for each text
    for (let i = 0; i < texts.length; i++) {
      const cached = this.cache.get(texts[i]);
      if (cached) {
        results[i] = cached;
      } else {
        uncachedIndices.push(i);
        uncachedTexts.push(texts[i]);
      }
    }

    // Generate embeddings for uncached texts
    if (uncachedTexts.length > 0) {
      const newEmbeddings = await this.provider.embedBatch(uncachedTexts);

      // Fill in results and cache
      for (let i = 0; i < uncachedIndices.length; i++) {
        const originalIndex = uncachedIndices[i];
        results[originalIndex] = newEmbeddings[i];
        this.cache.set(uncachedTexts[i], newEmbeddings[i]);
      }
    }

    logger.debug(
      `Embedding batch: ${texts.length} total, ${texts.length - uncachedTexts.length} cached`
    );

    return results as number[][];
  }

  getDimension(): number {
    return this.provider.getDimension();
  }

  getProvider(): EmbeddingProvider {
    return this.config.provider;
  }

  /** Clear the embedding cache */
  clearCache(): void {
    this.cache.clear();
    logger.info('Embedding cache cleared');
  }

  /** Get cache statistics */
  getCacheStats(): { size: number; provider: EmbeddingProvider; dimension: number } {
    return {
      size: this.cache.size(),
      provider: this.config.provider,
      dimension: this.getDimension(),
    };
  }
}

/** Singleton instance */
let embeddingServiceInstance: EmbeddingService | null = null;

/**
 * Get the embedding service instance
 */
export function getEmbeddingService(config?: Partial<EmbeddingConfig>): EmbeddingService {
  if (!embeddingServiceInstance) {
    embeddingServiceInstance = new EmbeddingService(config);
  }
  return embeddingServiceInstance;
}

/**
 * Reset the embedding service (useful for testing)
 */
export function resetEmbeddingService(): void {
  if (embeddingServiceInstance) {
    embeddingServiceInstance.clearCache();
    embeddingServiceInstance = null;
  }
}
