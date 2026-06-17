/**
 * Header component - app title with gradient
 */

import React from 'react';
import { Box, Text } from 'ink';
import Gradient from 'ink-gradient';
import BigText from 'ink-big-text';

interface HeaderProps {
  title?: string;
  subtitle?: string;
  compact?: boolean;
}

export function Header({
  title = 'Keepline',
  subtitle,
  compact = false
}: HeaderProps): React.ReactElement {
  if (compact) {
    return (
      <Box marginBottom={1}>
        <Gradient name="rainbow">
          <Text bold>{title}</Text>
        </Gradient>
        {subtitle && <Text color="gray"> - {subtitle}</Text>}
      </Box>
    );
  }

  return (
    <Box flexDirection="column" marginBottom={1}>
      <Gradient name="rainbow">
        <BigText text={title} font="chrome" />
      </Gradient>
      {subtitle && (
        <Box marginLeft={1}>
          <Text color="gray">{subtitle}</Text>
        </Box>
      )}
    </Box>
  );
}
