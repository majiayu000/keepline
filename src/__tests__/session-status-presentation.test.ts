import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { join } from 'path';
import {
  SESSION_STATUSES,
  SESSION_STATUS_PRESENTATION,
} from '../domain/session/index.js';
import { formatStatus } from '../lib/format.js';
import { themes } from '../ui/themes/index.js';

function sortedKeys(value: Record<string, unknown>): string[] {
  return Object.keys(value).sort();
}

describe('session status presentation coverage', () => {
  test('presents a stopped recoverable session as interrupted instead of destroyed', () => {
    expect(SESSION_STATUS_PRESENTATION.lost.label).toBe('Interrupted');
    expect(SESSION_STATUS_PRESENTATION.lost.shortLabel).toBe('INTR');
    expect(SESSION_STATUS_PRESENTATION.lost.shortLabel.length).toBeLessThanOrEqual(4);
    expect(formatStatus('lost')).toContain(SESSION_STATUS_PRESENTATION.lost.label);
  });

  test('routes remaining CLI copy through the Interrupted presentation label', () => {
    const watchSource = readFileSync(join(process.cwd(), 'src/cli/watch.ts'), 'utf8');
    const appSource = readFileSync(join(process.cwd(), 'src/ui/App.tsx'), 'utf8');
    expect(watchSource).toContain('SESSION_STATUS_PRESENTATION.lost.label');
    expect(watchSource).not.toMatch(/\$\{stats\.lost\} lost/);
    expect(appSource).toContain('SESSION_STATUS_PRESENTATION.lost.label');
    expect(appSource).not.toMatch(/label="lost"/);
    expect(appSource).not.toMatch(/lost sessions can be recovered/);
  });

  test('watch session table status column fits the Interrupted label', () => {
    const formatSource = readFileSync(join(process.cwd(), 'src/lib/format.ts'), 'utf8');
    const widths = formatSource.match(/colWidths:\s*\[([^\]]+)\]/);
    expect(widths).not.toBeNull();
    const statusWidth = Number(widths![1].split(',')[2]);
    expect(statusWidth).toBeGreaterThanOrEqual(
      SESSION_STATUS_PRESENTATION.lost.label.length + 2
    );
  });

  test('web status constants use the shared contract', () => {
    const constantsSource = readFileSync(
      join(process.cwd(), 'src/web/client/src/constants/index.ts'),
      'utf8'
    );

    expect(constantsSource).toContain('SESSION_STATUS_ORDER');
    for (const status of SESSION_STATUSES) {
      expect(constantsSource).toContain(
        `${status}: SESSION_STATUS_PRESENTATION.${status}.shortLabel`
      );
      expect(constantsSource).toContain(
        `${status}: SESSION_STATUS_PRESENTATION.${status}.icon`
      );
    }
  });

  test('ink themes define icons for every shared status', () => {
    for (const theme of Object.values(themes)) {
      expect(sortedKeys(theme.icons)).toEqual([...SESSION_STATUSES].sort());
    }
  });
});
