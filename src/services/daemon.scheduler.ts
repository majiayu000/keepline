/**
 * Daemon scheduler for periodic tasks
 */

import { config } from '../lib/config.js';
import { logger } from '../lib/logger.js';
import { syncSessions } from './session.service.js';
import { startHookServer, stopHookServer } from '../adapters/hook/server.js';
import { runMigrations } from '../db/migrations.js';
import { closeDatabase } from '../db/database.js';
import { emit } from '../lib/events.js';
import { initializeMemoryService } from './memory.service.js';

let scanInterval: NodeJS.Timeout | null = null;
let isRunning = false;

/** Run a single scan cycle */
async function runScanCycle(): Promise<void> {
  try {
    const result = await syncSessions();
    logger.debug(`Scan cycle: ${result.discovered} new, ${result.updated} updated, ${result.lost} lost`);
  } catch (error) {
    logger.error('Scan cycle failed', error);
    emit('error', { error: error as Error, context: 'scan_cycle' });
  }
}

/** Start the scheduler */
export async function startScheduler(): Promise<void> {
  if (isRunning) {
    logger.warn('Scheduler already running');
    return;
  }

  isRunning = true;
  const cfg = config.get();

  logger.info('Starting scheduler...');

  // Initialize database
  runMigrations();

  // Initialize memory service (must be before hook server)
  initializeMemoryService();

  // Start hook server
  await startHookServer();

  // Run initial scan
  await runScanCycle();

  // Start periodic scanning
  scanInterval = setInterval(runScanCycle, cfg.scanInterval);

  logger.info(`Scheduler started (scan interval: ${cfg.scanInterval}ms)`);
}

/** Stop the scheduler */
export async function stopScheduler(): Promise<void> {
  if (!isRunning) return;

  logger.info('Stopping scheduler...');

  // Stop periodic scanning
  if (scanInterval) {
    clearInterval(scanInterval);
    scanInterval = null;
  }

  // Stop hook server
  await stopHookServer();

  // Close database
  closeDatabase();

  isRunning = false;
  logger.info('Scheduler stopped');
}

/** Check if scheduler is running */
export function isSchedulerRunning(): boolean {
  return isRunning;
}

/** Run scheduler in foreground (for daemon process) */
export async function runDaemon(): Promise<void> {
  logger.info('Starting daemon...');

  // Handle graceful shutdown
  const shutdown = async (signal: string): Promise<void> => {
    logger.info(`Received ${signal}, shutting down...`);
    await stopScheduler();
    process.exit(0);
  };

  process.on('SIGTERM', () => shutdown('SIGTERM'));
  process.on('SIGINT', () => shutdown('SIGINT'));

  // Handle uncaught errors
  process.on('uncaughtException', (error) => {
    logger.error('Uncaught exception', error);
    emit('error', { error, context: 'uncaught_exception' });
  });

  process.on('unhandledRejection', (reason) => {
    const error = reason instanceof Error ? reason : new Error(String(reason));
    logger.error('Unhandled rejection', error);
    emit('error', { error, context: 'unhandled_rejection' });
  });

  // Start scheduler
  await startScheduler();

  // Keep process alive
  logger.info('Daemon running. Press Ctrl+C to stop.');
}

/** Trigger manual sync */
export async function triggerSync(): Promise<{
  discovered: number;
  updated: number;
  lost: number;
}> {
  return syncSessions();
}
