/**
 * SessionList component - clean, modern session list
 */

import React from 'react';
import { Box, Text } from 'ink';
import type { Session, SessionStatus } from '../../core/types.js';

interface SessionListProps {
  sessions: Session[];
}

const statusStyles: Record<SessionStatus, { icon: string; color: string; bg?: string }> = {
  running: { icon: '▶', color: 'green' },
  waiting: { icon: '◉', color: 'yellow' },
  idle: { icon: '○', color: 'blue' },
  lost: { icon: '✕', color: 'red' },
  completed: { icon: '✓', color: 'gray' },
};

function formatDir(dir: string): string {
  // Get last 2 path components
  const parts = dir.split('/').filter(Boolean);
  if (parts.length <= 2) return dir;
  return '…/' + parts.slice(-2).join('/');
}

function timeAgo(date: Date): string {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return 'now';
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d`;
}

function truncateTask(text: string, maxLen = 40): string {
  const clean = text.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim();
  if (clean.length <= maxLen) return clean;
  return clean.slice(0, maxLen - 1) + '…';
}

interface SessionRowProps {
  session: Session;
  index: number;
}

function SessionRow({ session, index }: SessionRowProps): React.ReactElement {
  const style = statusStyles[session.status];
  const task = truncateTask(session.title || session.initialPrompt || 'Unknown');
  const dir = formatDir(session.directory);
  const time = timeAgo(session.lastActiveAt);
  const num = String(index + 1).padStart(2);

  return (
    <Box>
      <Box width={4}><Text color="gray" dimColor>{num}</Text></Box>
      <Box width={2}><Text color={style.color}>{style.icon}</Text></Box>
      <Box width={22}><Text color="white">{dir}</Text></Box>
      <Box width={1}><Text> </Text></Box>
      <Box width={50}><Text color="gray">{task}</Text></Box>
      <Box width={5}><Text color="gray" dimColor>{time.padStart(4)}</Text></Box>
    </Box>
  );
}

export function SessionList({ sessions }: SessionListProps): React.ReactElement {
  // Group sessions by status
  const running = sessions.filter(s => s.status === 'running');
  const waiting = sessions.filter(s => s.status === 'waiting');
  const idle = sessions.filter(s => s.status === 'idle');
  const lost = sessions.filter(s => s.status === 'lost');

  const groups = [
    { title: 'Running', sessions: running, color: 'green' },
    { title: 'Waiting', sessions: waiting, color: 'yellow' },
    { title: 'Idle', sessions: idle, color: 'blue' },
    { title: 'Lost', sessions: lost, color: 'red' },
  ].filter(g => g.sessions.length > 0);

  let globalIndex = 0;

  return (
    <Box flexDirection="column">
      {groups.map((group, groupIndex) => (
        <Box key={group.title} flexDirection="column" marginBottom={groupIndex < groups.length - 1 ? 1 : 0}>
          {/* Group header */}
          <Box marginBottom={0} paddingLeft={1}>
            <Text color={group.color} bold>
              ━━ {group.title} ({group.sessions.length}) ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
            </Text>
          </Box>

          {/* Sessions in group */}
          {group.sessions.map((session) => {
            const currentIndex = globalIndex++;
            return (
              <SessionRow
                key={session.sessionId}
                session={session}
                index={currentIndex}
              />
            );
          })}
        </Box>
      ))}
    </Box>
  );
}
