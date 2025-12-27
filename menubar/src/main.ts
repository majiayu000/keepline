/**
 * Claude Quota Menubar - Electron Main Process
 *
 * Creates a menubar application that monitors Claude usage quota
 */

import { menubar } from 'menubar';
import { app, ipcMain, nativeTheme, Menu, shell, nativeImage } from 'electron';
import * as path from 'path';
import * as fs from 'fs';
import { getQuotaData, QuotaData } from './quota';

// Determine if we're in development
const isDev = process.env.NODE_ENV === 'development';

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

// Auto-refresh interval (30 seconds)
let refreshInterval: NodeJS.Timeout | null = null;
const REFRESH_INTERVAL = 30000;

mb.on('ready', () => {
  console.log('Claude Quota Menubar is ready');

  // Set up context menu for tray
  const contextMenu = Menu.buildFromTemplate([
    {
      label: 'Refresh',
      click: () => {
        mb.window?.webContents.send('refresh-quota');
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

  // Start auto-refresh
  startAutoRefresh();
});

mb.on('show', () => {
  // Refresh when shown
  mb.window?.webContents.send('refresh-quota');
});

mb.on('hide', () => {
  // Could pause refresh when hidden to save resources
});

// IPC handlers
ipcMain.handle('get-quota', async (): Promise<QuotaData | { error: string }> => {
  try {
    return await getQuotaData();
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

// Auto-refresh logic
function startAutoRefresh() {
  if (refreshInterval) {
    clearInterval(refreshInterval);
  }

  refreshInterval = setInterval(() => {
    if (mb.window?.isVisible()) {
      mb.window.webContents.send('refresh-quota');
    }
  }, REFRESH_INTERVAL);
}

// Handle app lifecycle
app.on('window-all-closed', () => {
  // Don't quit on window close for menubar apps
});

app.on('before-quit', () => {
  if (refreshInterval) {
    clearInterval(refreshInterval);
  }
});
