import Table from 'cli-table3';
import chalk from 'chalk';
import { runMigrations } from '../db/migrations.js';
import { syncSessions } from '../services/session.service.js';
import { getAggregatedSessions } from '../services/session.aggregator.js';
import {
  buildAttentionOverview,
  type AttentionOverview,
  type AttentionQueueItem,
} from '../services/attention.prioritizer.js';
import { logger } from '../lib/logger.js';

export interface OverviewOptions {
  all?: boolean;
  limit?: string;
  json?: boolean;
  highCostThreshold?: string;
  staleHours?: string;
}

export async function overviewCommand(options: OverviewOptions): Promise<void> {
  let parsedOptions: ParsedOverviewOptions;
  try {
    parsedOptions = parseOverviewOptions(options);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(chalk.red(message));
    process.exitCode = 1;
    return;
  }

  if (options.json) {
    logger.configure({ console: false });
  }

  runMigrations();
  if (!options.json) {
    process.stdout.write('\x1b[90mScanning sessions...\x1b[0m\n');
  }
  await syncSessions();

  const overview = buildAttentionOverview(getAggregatedSessions(), {
    includeCompleted: parsedOptions.includeCompleted,
    limit: parsedOptions.limit,
    highCostThreshold: parsedOptions.highCostThreshold,
    staleHours: parsedOptions.staleHours,
  });

  if (options.json) {
    console.log(JSON.stringify(serializeOverview(overview), null, 2));
    return;
  }

  process.stdout.write('\x1b[A\x1b[K');
  printOverview(overview);
}

export interface ParsedOverviewOptions {
  includeCompleted?: boolean;
  limit?: number;
  highCostThreshold?: number;
  staleHours?: number;
}

export function parseOverviewOptions(options: OverviewOptions): ParsedOverviewOptions {
  return {
    includeCompleted: options.all,
    limit: parseOptionalPositiveNumber(options.limit, 'limit'),
    highCostThreshold: parseOptionalPositiveNumber(
      options.highCostThreshold,
      'high-cost-threshold'
    ),
    staleHours: parseOptionalPositiveNumber(options.staleHours, 'stale-hours'),
  };
}

function parseOptionalPositiveNumber(value: string | undefined, name: string): number | undefined {
  if (value == null || value.trim() === '') return undefined;
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive number`);
  }
  return parsed;
}

function printOverview(overview: AttentionOverview): void {
  console.log('');
  console.log(chalk.bold('Keepline Attention Queue'));
  console.log(chalk.gray(`Generated: ${overview.generatedAt.toISOString()}`));
  console.log('');

  if (overview.items.length === 0) {
    console.log(chalk.gray('No sessions in the attention queue.'));
    console.log('');
    return;
  }

  const table = new Table({
    head: ['#', 'Score', 'Status', 'Client', 'Action', 'Session', 'Reasons'],
    style: { head: ['cyan'] },
    wordWrap: true,
    colWidths: [5, 8, 12, 10, 10, 32, 44],
  });

  for (const item of overview.items) {
    table.push([
      item.rank,
      item.score,
      item.status,
      item.client,
      item.recommendedAction,
      formatSessionLabel(item),
      item.reasons.map((reason) => reason.message).join('; ') || '-',
    ]);
  }

  console.log(table.toString());
  console.log('');
  console.log(
    chalk.gray(
      `Candidates: ${overview.stats.totalCandidates} | ` +
      `Needs attention: ${overview.stats.needingAttention} | ` +
      `Critical: ${overview.stats.critical} | Warning: ${overview.stats.warning}`
    )
  );
  console.log('');
}

function formatSessionLabel(item: AttentionQueueItem): string {
  const title = item.title || item.sessionId;
  return `${title}\n${chalk.gray(item.directory)}`;
}

function serializeOverview(overview: AttentionOverview) {
  return {
    generatedAt: overview.generatedAt.toISOString(),
    items: overview.items.map(serializeItem),
    stats: overview.stats,
  };
}

function serializeItem(item: AttentionQueueItem) {
  return {
    ...item,
    lastActiveAt: item.lastActiveAt.toISOString(),
  };
}
