import { describe, expect, test } from 'bun:test';
import { existsSync, readdirSync, readFileSync, statSync } from 'fs';
import { join, relative } from 'path';

const repoRoot = process.cwd();
const srcRoot = join(repoRoot, 'src');

const removedCompatibilityFiles = [
  'src/db/index.ts',
  'src/db/session.repository.ts',
  'src/lib/types.ts',
];

const forbiddenImportPatterns = [
  {
    pattern: /from\s+['"][^'"]*\/?lib\/types\.js['"]/,
    reason: 'Import session/domain types from src/domain instead of src/lib/types.ts',
  },
  {
    pattern: /from\s+['"][^'"]*\/?db\/index\.js['"]/,
    reason: 'Import sessionRepository from infrastructure/database/repositories directly',
  },
];

function sourceFiles(dir: string): string[] {
  const entries = readdirSync(dir);
  const files: string[] = [];

  for (const entry of entries) {
    if (entry === 'node_modules' || entry === 'dist') {
      continue;
    }

    const absolute = join(dir, entry);
    const stat = statSync(absolute);

    if (stat.isDirectory()) {
      files.push(...sourceFiles(absolute));
      continue;
    }

    if (/\.(ts|tsx)$/.test(entry)) {
      files.push(absolute);
    }
  }

  return files;
}

describe('architecture import boundaries', () => {
  test('removed session compatibility files stay removed', () => {
    for (const file of removedCompatibilityFiles) {
      expect(existsSync(join(repoRoot, file)), `${file} should not exist`).toBe(false);
    }
  });

  test('source files do not import removed session shims', () => {
    const violations: string[] = [];

    for (const file of sourceFiles(srcRoot)) {
      const relativePath = relative(repoRoot, file);
      const contents = readFileSync(file, 'utf8');

      for (const { pattern, reason } of forbiddenImportPatterns) {
        if (pattern.test(contents)) {
          violations.push(`${relativePath}: ${reason}`);
        }
      }
    }

    expect(violations).toEqual([]);
  });
});
