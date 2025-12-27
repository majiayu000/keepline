/**
 * Windows (Win32) Platform Strategy
 * Uses native Electron Tray API optimized for Windows
 */

import { BrowserWindow, Tray, nativeImage, screen } from 'electron';
import * as path from 'path';
import { IPlatformStrategy, PlatformConfig } from './types';

export class Win32Strategy implements IPlatformStrategy {
  readonly platform = 'win32' as const;
  private tray: Tray | null = null;

  getDefaultConfig(): PlatformConfig {
    return {
      iconPath: path.join(__dirname, '../../assets/icon.ico'),
      windowWidth: 320,
      windowHeight: 480,
      showDockIcon: false, // N/A for Windows
    };
  }

  createTray(iconPath: string): Tray {
    // Windows prefers .ico files, but can use .png
    let icon: Electron.NativeImage;

    // Try .ico first, fallback to .png
    const icoPath = iconPath.replace('.png', '.ico');
    try {
      icon = nativeImage.createFromPath(icoPath);
      if (icon.isEmpty()) {
        throw new Error('ICO empty');
      }
    } catch {
      icon = nativeImage.createFromPath(iconPath);
    }

    this.tray = new Tray(icon);
    this.tray.setToolTip('Claude Quota Monitor');
    return this.tray;
  }

  createWindow(config: PlatformConfig, indexUrl: string, preloadPath: string): BrowserWindow {
    const window = new BrowserWindow({
      width: config.windowWidth,
      height: config.windowHeight,
      show: false,
      frame: false,
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      alwaysOnTop: true,
      skipTaskbar: true,
      transparent: false,
      webPreferences: {
        preload: preloadPath,
        contextIsolation: true,
        nodeIntegration: false,
      },
    });

    window.loadURL(indexUrl);
    return window;
  }

  updateTrayDisplay(tray: Tray, percentage: number): void {
    // Windows doesn't support tray title, use tooltip instead
    const tooltip = `Claude Quota: ${Math.round(percentage)}% used`;
    tray.setToolTip(tooltip);

    // Optionally update icon to show percentage (could generate dynamic icon)
    // For now, just update tooltip
  }

  positionWindow(window: BrowserWindow, tray: Tray): void {
    const trayBounds = tray.getBounds();
    const windowBounds = window.getBounds();
    const display = screen.getDisplayMatching(trayBounds);
    const workArea = display.workArea;

    // Windows: tray is usually at bottom-right
    // Position window above the tray, aligned to right
    let x = Math.round(trayBounds.x + trayBounds.width / 2 - windowBounds.width / 2);
    let y = Math.round(trayBounds.y - windowBounds.height - 4);

    // If tray is at top (unusual), position below
    if (trayBounds.y < workArea.height / 2) {
      y = Math.round(trayBounds.y + trayBounds.height + 4);
    }

    // Ensure window stays within work area
    const maxX = workArea.x + workArea.width - windowBounds.width;
    const maxY = workArea.y + workArea.height - windowBounds.height;

    x = Math.max(workArea.x, Math.min(x, maxX));
    y = Math.max(workArea.y, Math.min(y, maxY));

    window.setPosition(x, y, false);
  }

  cleanup(): void {
    if (this.tray) {
      this.tray.destroy();
      this.tray = null;
    }
  }
}
