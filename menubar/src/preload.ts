/**
 * Preload Script
 *
 * Securely exposes main process APIs to the renderer
 */

import { contextBridge, ipcRenderer } from 'electron';

// Expose protected methods to the renderer
contextBridge.exposeInMainWorld('electronAPI', {
  // Get quota data
  getQuota: () => ipcRenderer.invoke('get-quota'),

  // Get system theme
  getTheme: () => ipcRenderer.invoke('get-theme'),

  // Open external URL
  openExternal: (url: string) => ipcRenderer.send('open-external', url),

  // Quit application
  quit: () => ipcRenderer.send('quit-app'),

  // Listen for refresh events
  onRefreshQuota: (callback: () => void) => {
    const handler = () => callback();
    ipcRenderer.on('refresh-quota', handler);
    return () => ipcRenderer.removeListener('refresh-quota', handler);
  },
});

// Type definitions for the exposed API
declare global {
  interface Window {
    electronAPI: {
      getQuota: () => Promise<import('./quota').QuotaData | { error: string }>;
      getTheme: () => Promise<'dark' | 'light'>;
      openExternal: (url: string) => void;
      quit: () => void;
      onRefreshQuota: (callback: () => void) => () => void;
    };
  }
}
