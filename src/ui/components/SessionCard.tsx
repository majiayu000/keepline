/**
 * SessionCard component - display a single session
 */

import React from 'react';
import { Text, Box } from 'ink';
import type { Session } from '../../core/types.js';
import { StatusBadge } from './StatusBadge.js';

interface SessionCardProps {
  session: Session;
  index: number;
  isSelected?: boolean;
}

/** Format directory path (truncate if too long) */
function formatDir(dir: string, maxLen = 45): string {
  if (dir.length <= maxLen) return dir;
  return '...' + dir.slice(-(maxLen - 3));
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
function truncate(text: string, maxLen = 60): string {
  if (text.length <= maxLen) return text;
  return text.slice(0, maxLen - 1) + '…';
}

export function SessionCard({ session, index, isSelected }: SessionCardProps): React.ReactElement {
  const borderColor = isSelected ? 'cyan' : 'gray';
  const task = session.title || session.initialPrompt || 'Unknown task';

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor={borderColor}
      paddingX={1}
      marginBottom={0}
    >
      {/* Header row */}
      <Box justifyContent="space-between">
        <Box>
          <Text color="cyan" bold>#{index + 1}</Text>
          <Text color="gray"> │ </Text>
          <Text color="white">{formatDir(session.directory)}</Text>
        </Box>
        <Box>
          <Text color="gray" dimColor>{timeAgo(session.lastActiveAt)}</Text>
        </Box>
      </Box>

      {/* Status and Task row */}
      <Box marginTop={0}>
        <StatusBadge status={session.status} />
        <Text color="gray"> │ </Text>
        <Text color="gray">{truncate(task)}</Text>
      </Box>
    </Box>
  );
}
