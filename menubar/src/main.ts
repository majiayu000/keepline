/**
 * Claude Quota Menubar - Electron Main Process
 * Using menubar library for reliable macOS tray support
 */

import { menubar } from 'menubar';
import { app, ipcMain, nativeTheme, Menu, shell } from 'electron';
import * as path from 'path';
import * as fs from 'fs';
import { getQuotaData, QuotaData } from './quota';

// Simple console logging
function log(message: string) {
  console.log(`[Claude Quota] ${message}`);
}

// Determine if we're in development
const isDev = process.env.NODE_ENV === 'development';
log(`Development mode: ${isDev}`);

// Dynamic refresh intervals based on usage level
const REFRESH_INTERVALS = {
  CRITICAL: 30 * 1000,   // 30s when >80%
  WARNING: 60 * 1000,    // 60s when 50-80%
  GOOD: 5 * 60 * 1000,   // 5min when <50%
  DEFAULT: 60 * 1000,
};

// Current state
let refreshInterval: NodeJS.Timeout | null = null;
let currentMaxUsage = 0;
let currentSessionUsage = 0;

// Get icon path - menubar library expects a PATH, not nativeImage
function getIconPath(): string {
  const iconPath = path.join(__dirname, '../assets/iconTemplate.png');
  log(`Icon path: ${iconPath}`);
  log(`Icon exists: ${fs.existsSync(iconPath)}`);
  return iconPath;
}

// Calculate refresh interval based on usage
function getRefreshInterval(maxUsage: number): number {
  if (maxUsage >= 80) return REFRESH_INTERVALS.CRITICAL;
  if (maxUsage >= 50) return REFRESH_INTERVALS.WARNING;
  if (maxUsage > 0) return REFRESH_INTERVALS.GOOD;
  return REFRESH_INTERVALS.DEFAULT;
}

// Format interval for logging
function formatInterval(ms: number): string {
  return ms >= 60000 ? `${ms / 60000} min` : `${ms / 1000} sec`;
}

// Create menubar instance - this is the WORKING approach
const mb = menubar({
  index: isDev
    ? 'http://localhost:5173'
    : `file://${path.join(__dirname, '../dist/renderer/index.html')}`,
  icon: getIconPath(),
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

// Update tray title (percentage display next to icon)
function updateTrayTitle(sessionUsage: number) {
  if (mb.tray) {
    const title = `${Math.round(sessionUsage)}%`;
    mb.tray.setTitle(title, { fontType: 'monospacedDigit' });
  }
}

// Start auto-refresh with dynamic interval
function startAutoRefresh() {
  stopAutoRefresh();
  const interval = getRefreshInterval(currentMaxUsage);
  log(`Auto-refresh started: every ${formatInterval(interval)} (usage: ${currentMaxUsage}%)`);

  refreshInterval = setInterval(() => {
    if (mb.window?.isVisible()) {
      mb.window.webContents.send('refresh-quota');
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

// Update refresh interval based on usage
function updateRefreshInterval(maxUsage: number) {
  const oldInterval = getRefreshInterval(currentMaxUsage);
  const newInterval = getRefreshInterval(maxUsage);
  currentMaxUsage = maxUsage;

  if (oldInterval !== newInterval && mb.window?.isVisible()) {
    log(`Usage changed to ${maxUsage}% - adjusting refresh`);
    startAutoRefresh();
  }
}

// Menubar ready event
mb.on('ready', async () => {
  log('Menubar is ready');

  // Fetch initial quota to set tray title
  try {
    const data = await getQuotaData();
    if ('connected' in data && data.connected && data.session) {
      currentSessionUsage = data.session.percentage;
      updateTrayTitle(currentSessionUsage);
      log(`Initial usage: ${currentSessionUsage}%`);
    }
  } catch (error) {
    log(`Failed to fetch initial quota: ${error}`);
  }

  // Set up context menu
  const contextMenu = Menu.buildFromTemplate([
    {
      label: 'Refresh Now',
      click: () => {
        mb.window?.webContents.send('refresh-quota');
      },
    },
    { type: 'separator' },
    {
      label: 'Open Claude',
      click: () => shell.openExternal('https://claude.ai'),
    },
    {
      label: 'Open Dashboard',
      click: () => shell.openExternal('http://localhost:3377'),
    },
    { type: 'separator' },
    {
      label: 'Quit',
      click: () => app.quit(),
    },
  ]);

  mb.tray.on('right-click', () => {
    mb.tray.popUpContextMenu(contextMenu);
  });

  // Start auto-refresh
  startAutoRefresh();

  log('Claude Quota Menubar is ready');
  log('Smart refresh enabled: only refreshes when window is visible');
  log(`Dynamic frequency: 30s (>80%), 60s (50-80%), 5min (<50%)`);
});

mb.on('show', () => {
  log('Window shown');
  mb.window?.webContents.send('refresh-quota');
  startAutoRefresh();
});

mb.on('hide', () => {
  log('Window hidden');
  // Keep refreshing in background to update tray title
});

// IPC handlers
ipcMain.handle('get-quota', async (): Promise<QuotaData | { error: string }> => {
  try {
    const data = await getQuotaData();

    if ('connected' in data && data.connected) {
      // Calculate max usage for dynamic refresh
      const usages = [
        data.session?.percentage ?? 0,
        data.weeklyTotal?.percentage ?? 0,
        data.weeklyOpus?.percentage ?? 0,
        data.weeklySonnet?.percentage ?? 0,
      ];
      const maxUsage = Math.max(...usages);
      updateRefreshInterval(maxUsage);

      // Update tray title with session usage
      const sessionUsage = data.session?.percentage ?? 0;
      if (sessionUsage !== currentSessionUsage) {
        currentSessionUsage = sessionUsage;
        updateTrayTitle(sessionUsage);
      }
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
