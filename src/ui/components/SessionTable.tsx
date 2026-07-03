/**
 * SessionTable component - beautiful session list table
 */

import React from 'react';
import { Box, Text } from 'ink';
import {
  SESSION_STATUS_PRESENTATION,
  type Session,
  type SessionStatus,
} from '../../domain/session/index.js';

interface SessionTableProps {
  sessions: Session[];
}

/** Status colors */
const statusColors: Record<SessionStatus, string> = {
  running: 'green',
  waiting: 'yellow',
  idle: 'blue',
  lost: 'red',
  completed: 'gray',
};

/** Status icons */
const statusIcons: Record<SessionStatus, string> = {
  running: '●',
  waiting: '◐',
  idle: '○',
  lost: '✖',
  completed: '✓',
};

/** Format directory path */
function formatDir(dir: string, maxLen = 32): string {
  if (dir.length <= maxLen) return dir;
  return '…' + dir.slice(-(maxLen - 1));
}

/** Format time ago */
function timeAgo(date: Date): string {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return 'now';
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
}

/** Truncate text */
function truncate(text: string, maxLen = 35): string {
  const clean = text.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim();
  if (clean.length <= maxLen) return clean;
  return clean.slice(0, maxLen - 1) + '…';
}

function TableRow({ session, index }: { session: Session; index: number }): React.ReactElement {
  const task = session.title || session.initialPrompt || 'Unknown task';
  const statusColor = statusColors[session.status];
  const statusIcon = statusIcons[session.status];
  const dir = formatDir(session.directory);
  const time = timeAgo(session.lastActiveAt);

  return (
    <Box>
      <Box width={4}>
        <Text color="gray">{String(index + 1).padStart(2)}</Text>
      </Box>
      <Box width={1}>
        <Text color="gray">│</Text>
      </Box>
      <Box width={34}>
        <Text color="white">{dir}</Text>
      </Box>
      <Box width={1}>
        <Text color="gray">│</Text>
      </Box>
      <Box width={10}>
        <Text color={statusColor}>{statusIcon} {SESSION_STATUS_PRESENTATION[session.status].shortLabel}</Text>
      </Box>
      <Box width={1}>
        <Text color="gray">│</Text>
      </Box>
      <Box width={37}>
        <Text color="gray">{truncate(task)}</Text>
      </Box>
      <Box width={1}>
        <Text color="gray">│</Text>
      </Box>
      <Box width={5}>
        <Text color="gray" dimColor>{time.padStart(4)}</Text>
      </Box>
    </Box>
  );
}

export function SessionTable({ sessions }: SessionTableProps): React.ReactElement {
  return (
    <Box flexDirection="column">
      {/* Header */}
      <Box borderStyle="single" borderColor="cyan" borderBottom={false}>
        <Box width={4}><Text color="cyan" bold> # </Text></Box>
        <Box width={1}><Text color="gray">│</Text></Box>
        <Box width={34}><Text color="cyan" bold>Directory</Text></Box>
        <Box width={1}><Text color="gray">│</Text></Box>
        <Box width={10}><Text color="cyan" bold>Status</Text></Box>
        <Box width={1}><Text color="gray">│</Text></Box>
        <Box width={37}><Text color="cyan" bold>Task</Text></Box>
        <Box width={1}><Text color="gray">│</Text></Box>
        <Box width={5}><Text color="cyan" bold>Time</Text></Box>
      </Box>

      {/* Divider */}
      <Box>
        <Text color="gray">├{'─'.repeat(4)}┼{'─'.repeat(34)}┼{'─'.repeat(10)}┼{'─'.repeat(37)}┼{'─'.repeat(5)}┤</Text>
      </Box>

      {/* Rows */}
      <Box flexDirection="column" borderStyle="single" borderColor="gray" borderTop={false}>
        {sessions.map((session, index) => (
          <TableRow key={session.sessionId} session={session} index={index} />
        ))}
      </Box>
    </Box>
  );
}
