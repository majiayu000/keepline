/**
 * UI Theme definitions
 */

export interface Theme {
  name: string;
  colors: {
    primary: string;
    secondary: string;
    success: string;
    warning: string;
    error: string;
    info: string;
    muted: string;
    border: string;
  };
  icons: {
    running: string;
    waiting: string;
    idle: string;
    lost: string;
    completed: string;
  };
  borders: {
    style: 'single' | 'double' | 'round' | 'bold' | 'classic' | 'arrow';
  };
}

// Style A: Minimal Clean (lazygit inspired)
export const minimalTheme: Theme = {
  name: 'minimal',
  colors: {
    primary: '#61afef',
    secondary: '#c678dd',
    success: '#98c379',
    warning: '#e5c07b',
    error: '#e06c75',
    info: '#56b6c2',
    muted: '#5c6370',
    border: '#3e4451',
  },
  icons: {
    running: '▸',
    waiting: '◦',
    idle: '·',
    lost: '×',
    completed: '✓',
  },
  borders: { style: 'single' },
};

// Style B: Dashboard Cards (btop inspired)
export const dashboardTheme: Theme = {
  name: 'dashboard',
  colors: {
    primary: '#7dcfff',
    secondary: '#bb9af7',
    success: '#9ece6a',
    warning: '#ff9e64',
    error: '#f7768e',
    info: '#2ac3de',
    muted: '#565f89',
    border: '#414868',
  },
  icons: {
    running: '●',
    waiting: '◐',
    idle: '○',
    lost: '✖',
    completed: '✔',
  },
  borders: { style: 'round' },
};

// Style C: Neon Cyberpunk
export const neonTheme: Theme = {
  name: 'neon',
  colors: {
    primary: '#00ffff',
    secondary: '#ff00ff',
    success: '#00ff00',
    warning: '#ffff00',
    error: '#ff0055',
    info: '#00ccff',
    muted: '#666699',
    border: '#333366',
  },
  icons: {
    running: '⚡',
    waiting: '⏳',
    idle: '◇',
    lost: '☠',
    completed: '★',
  },
  borders: { style: 'double' },
};

// Style D: macOS Inspired
export const macosTheme: Theme = {
  name: 'macos',
  colors: {
    primary: '#007aff',
    secondary: '#5856d6',
    success: '#34c759',
    warning: '#ff9500',
    error: '#ff3b30',
    info: '#5ac8fa',
    muted: '#8e8e93',
    border: '#c7c7cc',
  },
  icons: {
    running: '🟢',
    waiting: '🟡',
    idle: '⚪',
    lost: '🔴',
    completed: '✅',
  },
  borders: { style: 'round' },
};

export const themes = {
  minimal: minimalTheme,
  dashboard: dashboardTheme,
  neon: neonTheme,
  macos: macosTheme,
};

export type ThemeName = keyof typeof themes;
