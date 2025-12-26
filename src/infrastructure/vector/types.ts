/**
 * Vector Store Types
 *
 * Type definitions for the vector database layer.
 */

/** Observation category for memory classification */
export type ObservationCategory =
  | 'decision'    // Architecture/tech decisions
  | 'bugfix'      // Bug fixes
  | 'feature'     // New feature implementation
  | 'refactor'    // Code refactoring
  | 'discovery'   // Code understanding/exploration
  | 'change';     // File modifications

/** Observation stored in vector database */
export interface Observation {
  id: string;
  sessionId: string;
  content: string;
  category: ObservationCategory;
  files: string[];
  concepts: string[];
  timestamp: Date;
  tokenCount: number;
  compressed: boolean;
}

/** Observation with embedding vector */
export interface ObservationWithVector extends Observation {
  vector: number[];
}

/** Search result with similarity score */
export interface SearchResult {
  observation: Observation;
  score: number;  // 0-1, higher is more similar
  distance: number;  // Lower is more similar
}

/** Search options */
export interface SearchOptions {
  limit?: number;
  sessionId?: string;
  category?: ObservationCategory;
  minScore?: number;
  beforeDate?: Date;
  afterDate?: Date;
}

/** Vector store configuration */
export interface VectorStoreConfig {
  path: string;
  tableName: string;
  embeddingDimension: number;
}

/** Vector store interface */
export interface IVectorStore {
  /** Initialize the vector store */
  initialize(): Promise<void>;

  /** Close the connection */
  close(): Promise<void>;

  /** Insert an observation with its embedding */
  insert(observation: Observation, vector: number[]): Promise<void>;

  /** Insert multiple observations */
  insertBatch(observations: Array<{ observation: Observation; vector: number[] }>): Promise<void>;

  /** Search for similar observations */
  search(queryVector: number[], options?: SearchOptions): Promise<SearchResult[]>;

  /** Get observation by ID */
  getById(id: string): Promise<Observation | null>;

  /** Get observations by session ID */
  getBySessionId(sessionId: string): Promise<Observation[]>;

  /** Delete observation by ID */
  delete(id: string): Promise<boolean>;

  /** Delete all observations for a session */
  deleteBySessionId(sessionId: string): Promise<number>;

  /** Get total count of observations */
  count(): Promise<number>;
}
