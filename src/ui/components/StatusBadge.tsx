/**
 * StatusBadge component - colorful status indicator
 */

import React from 'react';
import { Text, Box } from 'ink';
import type { SessionStatus } from '../../core/types.js';

interface StatusBadgeProps {
  status: SessionStatus;
}

const statusConfig: Record<SessionStatus, { icon: string; color: string; bgColor?: string }> = {
  running: { icon: '●', color: 'green' },
  waiting: { icon: '◐', color: 'yellow' },
  idle: { icon: '○', color: 'blue' },
  lost: { icon: '✖', color: 'red' },
  completed: { icon: '✓', color: 'gray' },
};

const statusLabels: Record<SessionStatus, string> = {
  running: 'RUNNING',
  waiting: 'WAITING',
  idle: 'IDLE',
  lost: 'LOST',
  completed: 'DONE',
};

export function StatusBadge({ status }: StatusBadgeProps): React.ReactElement {
  const config = statusConfig[status];
  const label = statusLabels[status];

  return (
    <Box>
      <Text color={config.color}>
        {config.icon} {label}
      </Text>
    </Box>
  );
}
