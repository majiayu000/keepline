/**
 * Platform Strategy Factory
 * Selects the appropriate platform strategy based on the OS
 */

import { IPlatformStrategy } from './types';
import { DarwinStrategy } from './darwin';
import { Win32Strategy } from './win32';

export * from './types';
export { DarwinStrategy } from './darwin';
export { Win32Strategy } from './win32';

/**
 * Get the appropriate platform strategy for the current OS
 */
export function getPlatformStrategy(): IPlatformStrategy {
  switch (process.platform) {
    case 'darwin':
      return new DarwinStrategy();
    case 'win32':
      return new Win32Strategy();
    case 'linux':
      // Linux can use Win32 strategy as fallback (similar tray behavior)
      console.log('Linux detected, using Win32-compatible strategy');
      return new Win32Strategy();
    default:
      console.warn(`Unknown platform: ${process.platform}, defaulting to Win32 strategy`);
      return new Win32Strategy();
  }
}

/**
 * Check if running on macOS
 */
export function isDarwin(): boolean {
  return process.platform === 'darwin';
}

/**
 * Check if running on Windows
 */
export function isWindows(): boolean {
  return process.platform === 'win32';
}

/**
 * Check if running on Linux
 */
export function isLinux(): boolean {
  return process.platform === 'linux';
}
