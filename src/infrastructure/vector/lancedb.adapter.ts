/**
 * LanceDB Vector Store Adapter
 *
 * Implements the vector store interface using LanceDB for
 * efficient similarity search of memory observations.
 */

import * as lancedb from '@lancedb/lancedb';
import { join } from 'path';
import { mkdirSync, existsSync } from 'fs';
import { logger } from '../../lib/logger.js';
import { assertValidObservationId } from '../../lib/observation-id.js';
import { KEEPLINE_HOME } from '../../lib/paths.js';
import type {
  IVectorStore,
  Observation,
  SearchResult,
  SearchOptions,
  VectorStoreConfig,
  ObservationCategory,
} from './types.js';

/** Default configuration */
const DEFAULT_CONFIG: VectorStoreConfig = {
  path: join(KEEPLINE_HOME, 'lancedb'),
  tableName: 'observations',
  embeddingDimension: 1536, // OpenAI ada-002 / Voyage default
};

/** LanceDB row schema - using index signature for compatibility */
interface LanceDBRow {
  id: string;
  sessionId: string;
  content: string;
  category: string;
  files: string; // JSON string
  concepts: string; // JSON string
  timestamp: string; // ISO string
  tokenCount: number;
  compressed: boolean;
  vector: number[];
  [key: string]: string | number | boolean | number[];
}

/** Convert Observation to LanceDB row */
function toRow(observation: Observation, vector: number[]): LanceDBRow {
  return {
    id: observation.id,
    sessionId: observation.sessionId,
    content: observation.content,
    category: observation.category,
    files: JSON.stringify(observation.files),
    concepts: JSON.stringify(observation.concepts),
    timestamp: observation.timestamp.toISOString(),
    tokenCount: observation.tokenCount,
    compressed: observation.compressed,
    vector,
  };
}

/** Convert LanceDB row to Observation */
function fromRow(row: LanceDBRow): Observation {
  return {
    id: row.id,
    sessionId: row.sessionId,
    content: row.content,
    category: row.category as ObservationCategory,
    files: JSON.parse(row.files),
    concepts: JSON.parse(row.concepts),
    timestamp: new Date(row.timestamp),
    tokenCount: row.tokenCount,
    compressed: row.compressed,
  };
}

/** Build a LanceDB string literal only after the observation id has passed the safe allowlist. */
function safeObservationIdLiteral(id: string): string {
  assertValidObservationId(id);
  return `'${id}'`;
}

/** Build a LanceDB predicate only from a safe observation id literal. */
function observationIdPredicate(id: string): string {
  return `id = ${safeObservationIdLiteral(id)}`;
}

/**
 * LanceDB Vector Store implementation
 */
export class LanceDBVectorStore implements IVectorStore {
  private config: VectorStoreConfig;
  private db: lancedb.Connection | null = null;
  private table: lancedb.Table | null = null;
  private initialized = false;

  constructor(config: Partial<VectorStoreConfig> = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Initialize the vector store
   */
  async initialize(): Promise<void> {
    if (this.initialized) return;

    try {
      // Ensure directory exists
      if (!existsSync(this.config.path)) {
        mkdirSync(this.config.path, { recursive: true });
      }

      // Connect to LanceDB
      this.db = await lancedb.connect(this.config.path);

      // Check if table exists
      const tables = await this.db.tableNames();

      if (tables.includes(this.config.tableName)) {
        // Open existing table
        this.table = await this.db.openTable(this.config.tableName);
        logger.info(`Opened existing LanceDB table: ${this.config.tableName}`);
      } else {
        // Create new table with initial empty data
        // LanceDB requires at least one row to infer schema
        logger.info(`LanceDB table ${this.config.tableName} will be created on first insert`);
      }

      this.initialized = true;
      logger.info(`LanceDB initialized at ${this.config.path}`);
    } catch (error) {
      logger.error('Failed to initialize LanceDB', error);
      throw error;
    }
  }

  /**
   * Ensure table exists, create if needed
   */
  private async ensureTable(sampleRow: LanceDBRow): Promise<lancedb.Table> {
    if (this.table) return this.table;

    if (!this.db) {
      throw new Error('Database not initialized');
    }

    // Create table with sample data
    this.table = await this.db.createTable(this.config.tableName, [sampleRow]);
    logger.info(`Created LanceDB table: ${this.config.tableName}`);

    return this.table;
  }

  /**
   * Close the connection
   */
  async close(): Promise<void> {
    // LanceDB handles cleanup automatically
    this.db = null;
    this.table = null;
    this.initialized = false;
    logger.info('LanceDB connection closed');
  }

  /**
   * Insert an observation with its embedding
   */
  async insert(observation: Observation, vector: number[]): Promise<void> {
    if (!this.initialized) await this.initialize();

    const row = toRow(observation, vector);

    if (!this.table) {
      await this.ensureTable(row);
    } else {
      await this.table.add([row]);
    }

    logger.debug(`Inserted observation ${observation.id}`);
  }

  /**
   * Insert multiple observations
   */
  async insertBatch(
    items: Array<{ observation: Observation; vector: number[] }>
  ): Promise<void> {
    if (items.length === 0) return;
    if (!this.initialized) await this.initialize();

    const rows = items.map(({ observation, vector }) => toRow(observation, vector));

    if (!this.table) {
      await this.ensureTable(rows[0]);
      // First row already inserted, add the rest
      if (rows.length > 1) {
        await this.table!.add(rows.slice(1));
      }
    } else {
      await this.table.add(rows);
    }

    logger.debug(`Inserted ${items.length} observations`);
  }

  /**
   * Search for similar observations
   */
  async search(queryVector: number[], options: SearchOptions = {}): Promise<SearchResult[]> {
    if (!this.initialized) await this.initialize();
    if (!this.table) return [];

    const { limit = 10, sessionId, category, minScore = 0 } = options;

    // Build search query
    let query = this.table.vectorSearch(queryVector).limit(limit * 2); // Get extra for filtering

    // Execute search
    const results = await query.toArray();

    // Filter and map results
    const searchResults: SearchResult[] = [];

    for (const row of results) {
      const typedRow = row as unknown as LanceDBRow & { _distance: number };

      // Apply filters
      if (sessionId && typedRow.sessionId !== sessionId) continue;
      if (category && typedRow.category !== category) continue;

      // Calculate score (1 - normalized distance)
      const distance = typedRow._distance || 0;
      const score = 1 / (1 + distance); // Convert distance to similarity score

      if (score < minScore) continue;

      searchResults.push({
        observation: fromRow(typedRow),
        score,
        distance,
      });

      if (searchResults.length >= limit) break;
    }

    return searchResults;
  }

  /**
   * Get observation by ID
   */
  async getById(id: string): Promise<Observation | null> {
    const predicate = observationIdPredicate(id);
    if (!this.initialized) await this.initialize();
    if (!this.table) return null;

    const results = await this.table
      .query()
      .where(predicate)
      .limit(1)
      .toArray();

    if (results.length === 0) return null;

    return fromRow(results[0] as unknown as LanceDBRow);
  }

  /**
   * Get observations by session ID
   */
  async getBySessionId(sessionId: string): Promise<Observation[]> {
    if (!this.initialized) await this.initialize();
    if (!this.table) return [];

    const results = await this.table
      .query()
      .where(`sessionId = '${sessionId}'`)
      .toArray();

    return results.map((row) => fromRow(row as unknown as LanceDBRow));
  }

  /**
   * Delete observation by ID
   */
  async delete(id: string): Promise<boolean> {
    const predicate = observationIdPredicate(id);
    if (!this.initialized) await this.initialize();
    if (!this.table) return false;

    try {
      await this.table.delete(predicate);
      logger.debug(`Deleted observation ${id}`);
      return true;
    } catch (error) {
      logger.error(`Failed to delete observation ${id}`, error);
      return false;
    }
  }

  /**
   * Delete all observations for a session
   */
  async deleteBySessionId(sessionId: string): Promise<number> {
    if (!this.initialized) await this.initialize();
    if (!this.table) return 0;

    try {
      // Get count before delete
      const before = await this.table
        .query()
        .where(`sessionId = '${sessionId}'`)
        .toArray();

      await this.table.delete(`sessionId = '${sessionId}'`);
      logger.debug(`Deleted ${before.length} observations for session ${sessionId}`);

      return before.length;
    } catch (error) {
      logger.error(`Failed to delete observations for session ${sessionId}`, error);
      return 0;
    }
  }

  /**
   * Get total count of observations
   */
  async count(): Promise<number> {
    if (!this.initialized) await this.initialize();
    if (!this.table) return 0;

    const results = await this.table.query().toArray();
    return results.length;
  }
}

/** Singleton instance */
let vectorStoreInstance: LanceDBVectorStore | null = null;

/**
 * Get the vector store instance
 */
export function getVectorStore(): LanceDBVectorStore {
  if (!vectorStoreInstance) {
    vectorStoreInstance = new LanceDBVectorStore();
  }
  return vectorStoreInstance;
}

/**
 * Initialize the vector store
 */
export async function initializeVectorStore(): Promise<void> {
  const store = getVectorStore();
  await store.initialize();
}

/**
 * Close the vector store
 */
export async function closeVectorStore(): Promise<void> {
  if (vectorStoreInstance) {
    await vectorStoreInstance.close();
    vectorStoreInstance = null;
  }
}
