/**
 * Claude Quota Menubar - Electron Main Process
 *
 * Creates a menubar application that monitors Claude usage quota
 * Features:
 * - Smart refresh: only refreshes when window is visible
 * - Dynamic frequency: adjusts based on usage level
 */

import { menubar } from 'menubar';
import { app, ipcMain, nativeTheme, Menu, shell, nativeImage } from 'electron';
import * as path from 'path';
import * as fs from 'fs';
import { getQuotaData, QuotaData } from './quota';

// Determine if we're in development
const isDev = process.env.NODE_ENV === 'development';

// Dynamic refresh intervals based on usage level
const REFRESH_INTERVALS = {
  CRITICAL: 30 * 1000,    // 30 seconds when usage > 80%
  WARNING: 60 * 1000,     // 60 seconds when usage 50-80%
  GOOD: 5 * 60 * 1000,    // 5 minutes when usage < 50%
  DEFAULT: 60 * 1000,     // 60 seconds default
};

// Current state
let refreshInterval: NodeJS.Timeout | null = null;
let currentMaxUsage = 0;
let isWindowVisible = false;

// Get icon path, fallback to creating a simple icon if not found
function getIcon() {
  const iconPath = path.join(__dirname, '../assets/iconTemplate.png');
  if (fs.existsSync(iconPath)) {
    return iconPath;
  }

  // Create a simple 16x16 icon programmatically
  const icon = nativeImage.createEmpty();
  // Return empty icon - Electron will use default
  return icon;
}

// Calculate refresh interval based on usage
function getRefreshInterval(maxUsage: number): number {
  if (maxUsage >= 80) {
    return REFRESH_INTERVALS.CRITICAL;
  } else if (maxUsage >= 50) {
    return REFRESH_INTERVALS.WARNING;
  } else if (maxUsage > 0) {
    return REFRESH_INTERVALS.GOOD;
  }
  return REFRESH_INTERVALS.DEFAULT;
}

// Format interval for logging
function formatInterval(ms: number): string {
  if (ms >= 60000) {
    return `${ms / 60000} min`;
  }
  return `${ms / 1000} sec`;
}

// Create menubar instance
const mb = menubar({
  index: isDev
    ? 'http://localhost:5173'
    : `file://${path.join(__dirname, '../dist/renderer/index.html')}`,
  icon: getIcon(),
  browserWindow: {
    width: 320,
    height: 420,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  },
  preloadWindow: true,
  showDockIcon: false,
  tooltip: 'Claude Quota Monitor',
});

mb.on('ready', () => {
  console.log('Claude Quota Menubar is ready');
  console.log('Smart refresh enabled: only refreshes when window is visible');
  console.log('Dynamic frequency: 30s (>80%), 60s (50-80%), 5min (<50%)');

  // Set up context menu for tray
  const contextMenu = Menu.buildFromTemplate([
    {
      label: 'Refresh Now',
      click: () => {
        triggerRefresh();
      },
    },
    { type: 'separator' },
    {
      label: 'Open Claude',
      click: () => {
        shell.openExternal('https://claude.ai');
      },
    },
    {
      label: 'Open Dashboard',
      click: () => {
        shell.openExternal('http://localhost:3377');
      },
    },
    { type: 'separator' },
    {
      label: 'Quit',
      click: () => {
        app.quit();
      },
    },
  ]);

  mb.tray.on('right-click', () => {
    mb.tray.popUpContextMenu(contextMenu);
  });
});

// Smart refresh: start when window shows
mb.on('show', () => {
  isWindowVisible = true;
  console.log('Window shown - starting refresh');

  // Immediately refresh when shown
  triggerRefresh();

  // Start auto-refresh timer
  startAutoRefresh();
});

// Smart refresh: stop when window hides
mb.on('hide', () => {
  isWindowVisible = false;
  console.log('Window hidden - stopping refresh');

  // Stop auto-refresh to save resources
  stopAutoRefresh();
});

// Trigger a refresh
function triggerRefresh() {
  mb.window?.webContents.send('refresh-quota');
}

// Start auto-refresh with dynamic interval
function startAutoRefresh() {
  stopAutoRefresh(); // Clear any existing interval

  const interval = getRefreshInterval(currentMaxUsage);
  console.log(`Auto-refresh started: every ${formatInterval(interval)} (usage: ${currentMaxUsage}%)`);

  refreshInterval = setInterval(() => {
    if (isWindowVisible) {
      triggerRefresh();
    }
  }, interval);
}

// Stop auto-refresh
function stopAutoRefresh() {
  if (refreshInterval) {
    clearInterval(refreshInterval);
    refreshInterval = null;
  }
}

// Update refresh interval based on new usage data
function updateRefreshInterval(maxUsage: number) {
  const oldInterval = getRefreshInterval(currentMaxUsage);
  const newInterval = getRefreshInterval(maxUsage);

  currentMaxUsage = maxUsage;

  // Only restart if interval changed and window is visible
  if (oldInterval !== newInterval && isWindowVisible) {
    console.log(`Usage changed to ${maxUsage}% - adjusting refresh to ${formatInterval(newInterval)}`);
    startAutoRefresh();
  }
}

// IPC handlers
ipcMain.handle('get-quota', async (): Promise<QuotaData | { error: string }> => {
  try {
    const data = await getQuotaData();

    // Update dynamic refresh interval based on usage
    if ('connected' in data && data.connected) {
      const usages = [
        data.session?.percentage ?? 0,
        data.weeklyTotal?.percentage ?? 0,
        data.weeklyOpus?.percentage ?? 0,
        data.weeklySonnet?.percentage ?? 0,
      ];
      const maxUsage = Math.max(...usages);
      updateRefreshInterval(maxUsage);
    }

    return data;
  } catch (error) {
    console.error('Failed to get quota:', error);
    return { error: error instanceof Error ? error.message : 'Unknown error' };
  }
});

ipcMain.handle('get-theme', () => {
  return nativeTheme.shouldUseDarkColors ? 'dark' : 'light';
});

ipcMain.on('open-external', (_, url: string) => {
  shell.openExternal(url);
});

ipcMain.on('quit-app', () => {
  app.quit();
});

// Handle app lifecycle
app.on('window-all-closed', () => {
  // Don't quit on window close for menubar apps
});

app.on('before-quit', () => {
  stopAutoRefresh();
});
