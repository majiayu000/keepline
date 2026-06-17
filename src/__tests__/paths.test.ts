import { describe, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { getKeeplineDb, getKeeplineHome } from '../lib/paths.js';

describe('Keepline paths', () => {
  test('prefers KEEPLINE_HOME for storage paths', () => {
    const root = mkdtempSync(join(tmpdir(), 'keepline-paths-'));
    const previousHome = process.env.KEEPLINE_HOME;

    try {
      process.env.KEEPLINE_HOME = root;

      expect(getKeeplineHome()).toBe(root);
      expect(getKeeplineDb()).toBe(join(root, 'keepline.db'));
    } finally {
      if (previousHome === undefined) {
        delete process.env.KEEPLINE_HOME;
      } else {
        process.env.KEEPLINE_HOME = previousHome;
      }
      rmSync(root, { recursive: true, force: true });
    }
  });

  test('uses ~/.keepline by default', () => {
    const previousHome = process.env.KEEPLINE_HOME;
    try {
      delete process.env.KEEPLINE_HOME;

      expect(getKeeplineHome()).toEndWith('/.keepline');
      expect(getKeeplineDb()).toEndWith('/.keepline/keepline.db');
    } finally {
      if (previousHome === undefined) {
        delete process.env.KEEPLINE_HOME;
      } else {
        process.env.KEEPLINE_HOME = previousHome;
      }
    }
  });
});
