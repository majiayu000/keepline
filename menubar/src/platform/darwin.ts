/**
 * macOS (Darwin) Platform Strategy
 * Uses menubar library for reliable tray support on macOS
 */

import { app, BrowserWindow, Tray, nativeImage, screen } from 'electron';
import * as path from 'path';
import { IPlatformStrategy, PlatformConfig } from './types';

export class DarwinStrategy implements IPlatformStrategy {
  readonly platform = 'darwin' as const;
  private tray: Tray | null = null;

  getDefaultConfig(): PlatformConfig {
    return {
      iconPath: path.join(__dirname, '../../assets/iconTemplate.png'),
      windowWidth: 320,
      windowHeight: 480,
      showDockIcon: false,
    };
  }

  createTray(iconPath: string): Tray {
    const icon = nativeImage.createFromPath(iconPath);
    icon.setTemplateImage(true); // macOS template image for dark/light mode
    this.tray = new Tray(icon);
    this.tray.setToolTip('Claude Quota Monitor');
    return this.tray;
  }

  createWindow(config: PlatformConfig, indexUrl: string, preloadPath: string): BrowserWindow {
    // Hide dock icon on macOS
    if (!config.showDockIcon) {
      app.dock?.hide();
    }

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
      vibrancy: 'popover', // macOS vibrancy effect
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
    // macOS supports tray title next to icon
    const title = `${Math.round(percentage)}%`;
    tray.setTitle(title, { fontType: 'monospacedDigit' });
  }

  positionWindow(window: BrowserWindow, tray: Tray): void {
    const trayBounds = tray.getBounds();
    const windowBounds = window.getBounds();
    const display = screen.getDisplayMatching(trayBounds);

    // Position below tray icon, centered
    let x = Math.round(trayBounds.x + trayBounds.width / 2 - windowBounds.width / 2);
    let y = Math.round(trayBounds.y + trayBounds.height + 4);

    // Ensure window stays within screen bounds
    const maxX = display.bounds.x + display.bounds.width - windowBounds.width;
    const maxY = display.bounds.y + display.bounds.height - windowBounds.height;

    x = Math.max(display.bounds.x, Math.min(x, maxX));
    y = Math.max(display.bounds.y, Math.min(y, maxY));

    window.setPosition(x, y, false);
  }

  cleanup(): void {
    if (this.tray) {
      this.tray.destroy();
      this.tray = null;
    }
  }
}
