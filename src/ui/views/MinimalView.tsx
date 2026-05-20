/**
 * Style A: Minimal Clean View (lazygit/lazydocker inspired)
 * - Simple borders
 * - Compact layout
 * - Focus on content
 */

import React from 'react';
import { Box, Text } from 'ink';
import type { Session, SessionStatus } from '../../domain/session/index.js';

interface Props {
  sessions: Session[];
  stats: { total: number; running: number; waiting: number; idle: number; lost: number };
}

const icons: Record<SessionStatus, string> = {
  running: '▸',
  waiting: '◦',
  idle: '·',
  lost: '×',
  completed: '✓',
};

const colors: Record<SessionStatus, string> = {
  running: '#98c379',
  waiting: '#e5c07b',
  idle: '#61afef',
  lost: '#e06c75',
  completed: '#5c6370',
};

function formatPath(path: string): string {
  const parts = path.split('/').filter(Boolean);
  if (parts.length <= 2) return path;
  return `~/${parts.slice(-2).join('/')}`;
}

function timeAgo(date: Date): string {
  const s = Math.floor((Date.now() - date.getTime()) / 1000);
  if (s < 60) return 'now';
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}

function truncate(text: string, len = 40): string {
  const clean = text.replace(/\s+/g, ' ').trim();
  return clean.length <= len ? clean : clean.slice(0, len - 1) + '…';
}

export function MinimalView({ sessions, stats }: Props): React.ReactElement {
  return (
    <Box flexDirection="column" paddingX={1}>
      {/* Header - super minimal */}
      <Box marginBottom={1}>
        <Text color="#61afef" bold>tasker</Text>
        <Text color="#5c6370"> │ </Text>
        <Text color="#5c6370">{stats.total} sessions</Text>
        <Text color="#3e4451"> │ </Text>
        <Text color="#98c379">{stats.running}</Text>
        <Text color="#5c6370">/</Text>
        <Text color="#e5c07b">{stats.waiting}</Text>
        <Text color="#5c6370">/</Text>
        <Text color="#61afef">{stats.idle}</Text>
        <Text color="#5c6370">/</Text>
        <Text color="#e06c75">{stats.lost}</Text>
      </Box>

      {/* Divider */}
      <Box>
        <Text color="#3e4451">{'─'.repeat(80)}</Text>
      </Box>

      {/* Sessions list */}
      <Box flexDirection="column" marginTop={1}>
        {sessions.map((session, idx) => {
          const icon = icons[session.status];
          const color = colors[session.status];
          const task = truncate(session.title || session.initialPrompt || '—');
          const path = formatPath(session.directory);
          const time = timeAgo(session.lastActiveAt);

          return (
            <Box key={session.sessionId}>
              <Text color="#5c6370">{String(idx + 1).padStart(2)} </Text>
              <Text color={color}>{icon} </Text>
              <Text color="#abb2bf">{path.padEnd(24).slice(0, 24)} </Text>
              <Text color="#5c6370">{task.padEnd(42).slice(0, 42)} </Text>
              <Text color="#3e4451">{time.padStart(4)}</Text>
            </Box>
          );
        })}
      </Box>

      {/* Footer */}
      {stats.lost > 0 && (
        <Box marginTop={1}>
          <Text color="#3e4451">{'─'.repeat(80)}</Text>
        </Box>
      )}
      {stats.lost > 0 && (
        <Box>
          <Text color="#5c6370">recover: </Text>
          <Text color="#61afef">tasker r {'<n>'}</Text>
        </Box>
      )}
    </Box>
  );
}
