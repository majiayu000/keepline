/**
 * List command - show all sessions with beautiful Ink UI
 */

import React from 'react';
import { render } from 'ink';
import { runMigrations } from '../db/migrations.js';
import { syncSessions } from '../services/session.service.js';
import { getAggregatedSessions, filterSessions, sortSessions, getSessionStats } from '../services/session.aggregator.js';
import { App } from '../ui/App.js';
import { MinimalView, DashboardView, NeonView, MacOSView, CyberpunkView } from '../ui/views/index.js';
import type { SessionStatus } from '../domain/session/index.js';
import type { ViewStyle } from '../ui/views/index.js';

interface ListOptions {
  status?: string;
  directory?: string;
  limit?: string;
  all?: boolean;
  style?: ViewStyle;
}

export async function listCommand(options: ListOptions): Promise<void> {
  // Initialize database
  runMigrations();

  // Sync sessions before rendering the list.
  process.stdout.write('\x1b[90mScanning sessions...\x1b[0m\n');
  await syncSessions();

  // Get sessions
  let sessions = getAggregatedSessions();

  // Apply filters
  const statusFilter = options.status
    ? (options.status.split(',') as SessionStatus[])
    : undefined;

  // By default, hide completed sessions unless --all is passed
  const defaultStatusFilter: SessionStatus[] = ['running', 'waiting', 'idle', 'lost'];

  sessions = filterSessions(sessions, {
    status: options.all ? undefined : (statusFilter || defaultStatusFilter),
    directory: options.directory,
    limit: options.limit ? parseInt(options.limit, 10) : undefined,
  });

  // Sort by last active (most recent first)
  sessions = sortSessions(sessions, { field: 'lastActiveAt', order: 'desc' });

  // Get stats
  const stats = getSessionStats(sessions);

  // Clear the "Scanning" message
  process.stdout.write('\x1b[A\x1b[K');

  // Select view based on style option (cyber is default)
  const style = options.style || 'cyber';

  let ViewComponent: React.FC<{ sessions: typeof sessions; stats: typeof stats }>;

  switch (style) {
    case 'minimal':
      ViewComponent = MinimalView;
      break;
    case 'dashboard':
      ViewComponent = DashboardView;
      break;
    case 'neon':
      ViewComponent = NeonView;
      break;
    case 'macos':
      ViewComponent = MacOSView;
      break;
    case 'cyber':
      ViewComponent = CyberpunkView;
      break;
    case 'default':
      ViewComponent = ({ sessions, stats }) => <App sessions={sessions} stats={stats} />;
      break;
    default:
      ViewComponent = CyberpunkView;
  }

  // Render with Ink
  const { unmount } = render(<ViewComponent sessions={sessions} stats={stats} />);

  // Wait briefly then unmount (static output)
  await new Promise(resolve => setTimeout(resolve, 100));
  unmount();
}
