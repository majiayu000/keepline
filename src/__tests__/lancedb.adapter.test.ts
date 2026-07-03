import { describe, expect, test } from 'bun:test';
import {
  closeVectorStore,
  getVectorStore,
  LanceDBVectorStore,
} from '../infrastructure/vector/lancedb.adapter.js';
import { resetEmbeddingService } from '../infrastructure/vector/embedding.service.js';
import type { Observation } from '../infrastructure/vector/types.js';

type MockQuery = {
  limit: (limit: number) => MockQuery;
  toArray: () => Promise<unknown[]>;
};

type MockTable = {
  addCalls: unknown[][];
  deleteCalls: string[];
  countRowsCalls: Array<string | undefined>;
  queryCalls: number;
  add: (rows: unknown[]) => Promise<void>;
  delete: (predicate: string) => Promise<void>;
  countRows?: (predicate?: string) => Promise<number>;
  query: () => MockQuery;
};

type StoreInternals = {
  config: { embeddingDimension: number };
  initialized: boolean;
  table: MockTable;
  tableVectorDimension: number | null;
};

function observation(id = 'observe-1'): Observation {
  return {
    id,
    sessionId: 'session-1',
    content: 'Remember this',
    category: 'decision',
    files: [],
    concepts: [],
    timestamp: new Date('2026-07-03T00:00:00.000Z'),
    tokenCount: 3,
    compressed: false,
  };
}

function mockTable(sampleRows: unknown[] = []): MockTable {
  const table: MockTable = {
    addCalls: [],
    deleteCalls: [],
    countRowsCalls: [],
    queryCalls: 0,
    async add(rows) {
      table.addCalls.push(rows);
    },
    async delete(predicate) {
      table.deleteCalls.push(predicate);
    },
    async countRows(predicate) {
      table.countRowsCalls.push(predicate);
      return 3;
    },
    query() {
      table.queryCalls += 1;
      return {
        limit() {
          return this;
        },
        async toArray() {
          return sampleRows;
        },
      };
    },
  };

  return table;
}

function initializedStore(table: MockTable, dimension = 3): LanceDBVectorStore {
  const store = new LanceDBVectorStore({
    path: '/tmp/keepline-lancedb-test',
    tableName: 'observations',
    embeddingDimension: dimension,
  });
  const internals = store as unknown as StoreInternals;
  internals.initialized = true;
  internals.table = table;
  internals.tableVectorDimension = null;
  return store;
}

function vector(dimension: number): number[] {
  return Array.from({ length: dimension }, (_, index) => index / dimension);
}

describe('LanceDBVectorStore vector dimensions', () => {
  test('singleton uses the active embedding provider dimension', async () => {
    const previousOpenAiKey = process.env.OPENAI_API_KEY;
    const previousVoyageKey = process.env.VOYAGE_API_KEY;

    try {
      delete process.env.OPENAI_API_KEY;
      delete process.env.VOYAGE_API_KEY;
      resetEmbeddingService();
      await closeVectorStore();

      const store = getVectorStore();

      expect((store as unknown as StoreInternals).config.embeddingDimension).toBe(384);
    } finally {
      if (previousOpenAiKey === undefined) {
        delete process.env.OPENAI_API_KEY;
      } else {
        process.env.OPENAI_API_KEY = previousOpenAiKey;
      }
      if (previousVoyageKey === undefined) {
        delete process.env.VOYAGE_API_KEY;
      } else {
        process.env.VOYAGE_API_KEY = previousVoyageKey;
      }
      resetEmbeddingService();
      await closeVectorStore();
    }
  });

  test('rejects a single vector with the wrong dimension before writing', async () => {
    const table = mockTable();
    const store = initializedStore(table, 1536);

    await expect(store.insert(observation(), vector(384))).rejects.toThrow(
      'Vector dimension mismatch: expected 1536, got 384'
    );

    expect(table.addCalls).toHaveLength(0);
  });

  test('rejects mixed batch dimensions before a partial batch write', async () => {
    const table = mockTable();
    const store = initializedStore(table, 384);

    await expect(
      store.insertBatch([
        { observation: observation('observe-1'), vector: vector(384) },
        { observation: observation('observe-2'), vector: vector(1536) },
      ])
    ).rejects.toThrow('Vector dimension mismatch: expected 384, got 1536');

    expect(table.addCalls).toHaveLength(0);
  });

  test('rejects writes when an existing table has a different vector dimension', async () => {
    const table = mockTable([{ vector: vector(384) }]);
    const store = initializedStore(table, 1536);

    await expect(store.insert(observation(), vector(1536))).rejects.toThrow(
      'LanceDB table vector dimension mismatch: expected 1536, got 384'
    );

    expect(table.addCalls).toHaveLength(0);
  });
});

describe('LanceDBVectorStore bulk maintenance', () => {
  test('counts rows without reading the full table', async () => {
    const table = mockTable([{ vector: [1, 2, 3] }]);
    const store = initializedStore(table);

    await expect(store.count()).resolves.toBe(3);

    expect(table.countRowsCalls).toEqual([undefined]);
    expect(table.queryCalls).toBe(0);
  });

  test('deletes by session id with countRows and predicate delete', async () => {
    const table = mockTable([{ vector: [1, 2, 3] }]);
    const store = initializedStore(table);

    await expect(store.deleteBySessionId('session-1')).resolves.toBe(3);

    expect(table.countRowsCalls).toEqual(["sessionId = 'session-1'"]);
    expect(table.deleteCalls).toEqual(["sessionId = 'session-1'"]);
    expect(table.queryCalls).toBe(0);
  });

  test('rejects unsafe session ids before predicate delete', async () => {
    const table = mockTable([{ vector: [1, 2, 3] }]);
    const store = initializedStore(table);

    await expect(store.deleteBySessionId("session' OR 1=1")).rejects.toThrow(
      'Invalid session ID format'
    );

    expect(table.countRowsCalls).toEqual([]);
    expect(table.deleteCalls).toEqual([]);
  });
});
