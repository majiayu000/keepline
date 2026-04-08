/**
 * Main Ink App component for list view
 */

import React from 'react';
import { Box, Text } from 'ink';
import Gradient from 'ink-gradient';
import type { Session } from '../lib/types.js';
import { SessionList } from './components/index.js';

interface SessionStats {
  total: number;
  running: number;
  waiting: number;
  idle: number;
  lost: number;
  completed: number;
}

interface AppProps {
  sessions: Session[];
  stats: SessionStats;
}

function StatBadge({ label, value, color }: { label: string; value: number; color: string }): React.ReactElement {
  return (
    <Box marginRight={2}>
      <Text color={color} bold>{value}</Text>
      <Text color="gray"> {label}</Text>
    </Box>
  );
}

export function App({ sessions, stats }: AppProps): React.ReactElement {
  const hasLostSessions = stats.lost > 0;

  return (
    <Box flexDirection="column" paddingX={1} paddingY={1}>
      {/* Header */}
      <Box marginBottom={1}>
        <Gradient name="pastel">
          <Text bold>◆ CLAUDE HUB</Text>
        </Gradient>
        <Text color="gray"> — Claude Code session control center</Text>
      </Box>

      {/* Stats bar */}
      <Box marginBottom={1} paddingX={1}>
        <Text color="white" bold>{stats.total}</Text>
        <Text color="gray"> sessions │ </Text>
        <StatBadge label="running" value={stats.running} color="green" />
        <StatBadge label="waiting" value={stats.waiting} color="yellow" />
        <StatBadge label="idle" value={stats.idle} color="blue" />
        <StatBadge label="lost" value={stats.lost} color="red" />
      </Box>

      {sessions.length === 0 ? (
        <Box paddingX={1} paddingY={1}>
          <Text color="yellow">No sessions found.</Text>
        </Box>
      ) : (
        <SessionList sessions={sessions} />
      )}

      {hasLostSessions && (
        <Box marginTop={1} paddingX={1}>
          <Text color="yellow">💡 </Text>
          <Text color="gray">{stats.lost} lost sessions can be recovered: </Text>
          <Text color="cyan" bold>claude-hub recover {'<n>'}</Text>
        </Box>
      )}
    </Box>
  );
}
