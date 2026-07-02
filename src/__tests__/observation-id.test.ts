import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';
import { LanceDBVectorStore } from '../infrastructure/vector/lancedb.adapter.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase } from '../infrastructure/database/sqlite.js';
import { setupUser } from '../services/auth.service.js';
import memory from '../web/api/routes/memory.js';

const INJECTION_ID = "x' OR '1'='1";
const MISSING_VALID_ID = '550e8400-e29b-41d4-a716-446655440000';

describe('observation id hardening', () => {
  const tempDirs: string[] = [];

  beforeEach(() => {
    resetDatabase();
  });

  afterEach(() => {
    closeDatabase();
    for (const dir of tempDirs.splice(0)) {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  function vectorStoreWithTempPath(): LanceDBVectorStore {
    const path = mkdtempSync(join(tmpdir(), 'keepline-gh74-lancedb-'));
    tempDirs.push(path);
    return new LanceDBVectorStore({ path });
  }

  test('memory GET route rejects injection-shaped observation IDs before vector access', async () => {
    const { token } = await setupUser('alice', 'password123');

    const response = await memory.fetch(new Request(
      `http://localhost/observations/${encodeURIComponent(INJECTION_ID)}`,
      { headers: { Authorization: `Bearer ${token}` } }
    ));

    expect(response.status).toBe(400);
    const body = await response.json() as { success: boolean; error: string };
    expect(body).toEqual({
      success: false,
      error: 'Invalid observation ID format',
    });
  });

  test('memory DELETE route rejects injection-shaped observation IDs before vector access', async () => {
    const { token } = await setupUser('bob', 'password123');

    const response = await memory.fetch(new Request(
      `http://localhost/observations/${encodeURIComponent(INJECTION_ID)}`,
      {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
      }
    ));

    expect(response.status).toBe(400);
    const body = await response.json() as { success: boolean; error: string };
    expect(body).toEqual({
      success: false,
      error: 'Invalid observation ID format',
    });
  });

  test('LanceDBVectorStore rejects unsafe getById IDs before initialization', async () => {
    const store = vectorStoreWithTempPath();

    await expect(store.getById(INJECTION_ID)).rejects.toThrow('Invalid observation ID format');
  });

  test('LanceDBVectorStore rejects unsafe delete IDs before initialization', async () => {
    const store = vectorStoreWithTempPath();

    await expect(store.delete(INJECTION_ID)).rejects.toThrow('Invalid observation ID format');
  });

  test('LanceDBVectorStore preserves not-found semantics for safe missing IDs', async () => {
    const store = vectorStoreWithTempPath();

    await expect(store.getById(MISSING_VALID_ID)).resolves.toBeNull();
    await expect(store.delete(MISSING_VALID_ID)).resolves.toBe(false);
  });
});
