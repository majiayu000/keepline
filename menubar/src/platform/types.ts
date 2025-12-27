/**
 * Platform Strategy Pattern - Type Definitions
 */

import { BrowserWindow, Tray } from 'electron';

// Platform-specific configuration
export interface PlatformConfig {
  iconPath: string;
  windowWidth: number;
  windowHeight: number;
  showDockIcon: boolean;
}

// Platform strategy interface
export interface IPlatformStrategy {
  readonly platform: 'darwin' | 'win32' | 'linux';

  // Initialize platform-specific tray
  createTray(iconPath: string): Tray;

  // Create and configure the window
  createWindow(config: PlatformConfig, indexUrl: string, preloadPath: string): BrowserWindow;

  // Update tray display (title/tooltip)
  updateTrayDisplay(tray: Tray, percentage: number): void;

  // Position window relative to tray
  positionWindow(window: BrowserWindow, tray: Tray): void;

  // Get default config for this platform
  getDefaultConfig(): PlatformConfig;

  // Platform-specific cleanup
  cleanup(): void;
}

// Tray event callbacks
export interface TrayEventCallbacks {
  onClick: () => void;
  onRightClick: () => void;
}
