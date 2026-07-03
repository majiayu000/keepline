/**
 * StatusBadge component - colorful status indicator
 */

import React from 'react';
import { Text, Box } from 'ink';
import {
  SESSION_STATUS_PRESENTATION,
  type SessionStatus,
} from '../../domain/session/index.js';

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

export function StatusBadge({ status }: StatusBadgeProps): React.ReactElement {
  const config = statusConfig[status];
  const label = SESSION_STATUS_PRESENTATION[status].shortLabel;

  return (
    <Box>
      <Text color={config.color}>
        {config.icon} {label}
      </Text>
    </Box>
  );
}
