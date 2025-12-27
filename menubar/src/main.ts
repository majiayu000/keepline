/**
 * Claude Quota Menubar - Electron Main Process
 * Cross-platform support using strategy pattern
 */

import { menubar, Menubar } from 'menubar';
import { app, ipcMain, nativeTheme, Menu, shell, Tray, BrowserWindow } from 'electron';
import * as path from 'path';
import * as fs from 'fs';
import { getQuotaData, QuotaData } from './quota';
import { getPlatformStrategy, isDarwin, IPlatformStrategy, PlatformConfig } from './platform';

// Simple console logging
function log(message: string) {
  console.log(`[Claude Quota] ${message}`);
}

// Determine if we're in development
const isDev = process.env.NODE_ENV === 'development';
log(`Development mode: ${isDev}`);
log(`Platform: ${process.platform}`);

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

// Platform-specific instances
let mb: Menubar | null = null;
let tray: Tray | null = null;
let mainWindow: BrowserWindow | null = null;
let platformStrategy: IPlatformStrategy | null = null;

// Get icon path
function getIconPath(): string {
  const iconName = isDarwin() ? 'iconTemplate.png' : 'icon.ico';
  const iconPath = path.join(__dirname, '../assets', iconName);
  log(`Icon path: ${iconPath}`);
  log(`Icon exists: ${fs.existsSync(iconPath)}`);
  return iconPath;
}

// Get index URL
function getIndexUrl(): string {
  return isDev
    ? 'http://localhost:5173'
    : `file://${path.join(__dirname, '../dist/renderer/index.html')}`;
}

// Get preload path
function getPreloadPath(): string {
  return path.join(__dirname, 'preload.js');
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

// Update tray display based on platform
function updateTrayDisplay(sessionUsage: number) {
  if (isDarwin() && mb?.tray) {
    const title = `${Math.round(sessionUsage)}%`;
    mb.tray.setTitle(title, { fontType: 'monospacedDigit' });
  } else if (tray && platformStrategy) {
    platformStrategy.updateTrayDisplay(tray, sessionUsage);
  }
}

// Get current window
function getWindow(): BrowserWindow | undefined {
  return isDarwin() ? mb?.window : mainWindow ?? undefined;
}

// Check if window is visible
function isWindowVisible(): boolean {
  const win = getWindow();
  return win?.isVisible() ?? false;
}

// Send message to renderer
function sendToRenderer(channel: string, ...args: unknown[]) {
  const win = getWindow();
  if (win) {
    win.webContents.send(channel, ...args);
  }
}

// Start auto-refresh with dynamic interval
function startAutoRefresh() {
  stopAutoRefresh();
  const interval = getRefreshInterval(currentMaxUsage);
  log(`Auto-refresh started: every ${formatInterval(interval)} (usage: ${currentMaxUsage}%)`);

  refreshInterval = setInterval(() => {
    if (isWindowVisible()) {
      sendToRenderer('refresh-quota');
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

  if (oldInterval !== newInterval && isWindowVisible()) {
    log(`Usage changed to ${maxUsage}% - adjusting refresh`);
    startAutoRefresh();
  }
}

// Create context menu
function createContextMenu(): Menu {
  return Menu.buildFromTemplate([
    {
      label: 'Refresh Now',
      click: () => sendToRenderer('refresh-quota'),
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
}

// Fetch initial quota data
async function fetchInitialQuota() {
  try {
    const data = await getQuotaData();
    if ('connected' in data && data.connected && data.session) {
      currentSessionUsage = data.session.percentage;
      updateTrayDisplay(currentSessionUsage);
      log(`Initial usage: ${currentSessionUsage}%`);
    }
  } catch (error) {
    log(`Failed to fetch initial quota: ${error}`);
  }
}

// Initialize for macOS using menubar library
function initDarwin() {
  log('Initializing for macOS using menubar library');

  mb = menubar({
    index: getIndexUrl(),
    icon: getIconPath(),
    browserWindow: {
      width: 320,
      height: 480,
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      alwaysOnTop: true,
      skipTaskbar: true,
      webPreferences: {
        preload: getPreloadPath(),
        contextIsolation: true,
        nodeIntegration: false,
      },
    },
    preloadWindow: true,
    showDockIcon: false,
    tooltip: 'Claude Quota Monitor',
  });

  mb.on('ready', async () => {
    log('Menubar is ready');
    await fetchInitialQuota();

    const contextMenu = createContextMenu();
    mb!.tray.on('right-click', () => {
      mb!.tray.popUpContextMenu(contextMenu);
    });

    // Hide window when clicking outside (blur event)
    mb!.window?.on('blur', () => {
      if (mb!.window?.isVisible()) {
        mb!.hideWindow();
        log('Window hidden (blur)');
      }
    });

    startAutoRefresh();
    log('Claude Quota Menubar is ready (macOS)');
  });

  mb.on('show', () => {
    log('Window shown');
    sendToRenderer('refresh-quota');
    startAutoRefresh();
  });

  mb.on('hide', () => {
    log('Window hidden');
  });
}

// Initialize for Windows/Linux using native Electron Tray
function initWin32() {
  log('Initializing for Windows/Linux using native Electron Tray');

  platformStrategy = getPlatformStrategy();
  const config = platformStrategy.getDefaultConfig();

  app.whenReady().then(async () => {
    log('App is ready');

    // Create tray
    tray = platformStrategy!.createTray(getIconPath());
    const contextMenu = createContextMenu();

    tray.on('click', () => {
      if (mainWindow) {
        if (mainWindow.isVisible()) {
          mainWindow.hide();
        } else {
          platformStrategy!.positionWindow(mainWindow, tray!);
          mainWindow.show();
          mainWindow.focus();
          sendToRenderer('refresh-quota');
        }
      }
    });

    tray.on('right-click', () => {
      tray!.popUpContextMenu(contextMenu);
    });

    // Create window
    mainWindow = platformStrategy!.createWindow(config, getIndexUrl(), getPreloadPath());

    mainWindow.on('blur', () => {
      // Hide window when it loses focus (similar to macOS menubar behavior)
      if (mainWindow?.isVisible()) {
        mainWindow.hide();
      }
    });

    mainWindow.on('show', () => {
      log('Window shown');
      startAutoRefresh();
    });

    mainWindow.on('hide', () => {
      log('Window hidden');
    });

    await fetchInitialQuota();
    startAutoRefresh();
    log('Claude Quota Menubar is ready (Windows/Linux)');
  });
}

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

      // Update tray display with session usage
      const sessionUsage = data.session?.percentage ?? 0;
      if (sessionUsage !== currentSessionUsage) {
        currentSessionUsage = sessionUsage;
        updateTrayDisplay(sessionUsage);
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
  if (platformStrategy) {
    platformStrategy.cleanup();
  }
});

// Initialize based on platform
if (isDarwin()) {
  initDarwin();
} else {
  initWin32();
}
