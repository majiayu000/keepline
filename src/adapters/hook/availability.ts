import { getHookStatus } from './installer.js';
import { getHookServerUrl, isHookServerRunning } from './server.js';

export interface HookAvailability {
  installed: boolean;
  receiverRunning: boolean;
  degraded: boolean;
  settingsPath: string;
  hookCommand: string;
  hookServerUrl: string;
}

export function buildHookAvailability(input: {
  installed: boolean;
  receiverRunning: boolean;
  settingsPath: string;
  hookCommand: string;
  hookServerUrl: string;
}): HookAvailability {
  return {
    ...input,
    degraded: input.installed && !input.receiverRunning,
  };
}

export function getHookAvailability(): HookAvailability {
  const status = getHookStatus();
  return buildHookAvailability({
    installed: status.installed,
    receiverRunning: isHookServerRunning(),
    settingsPath: status.settingsPath,
    hookCommand: status.hookCommand,
    hookServerUrl: getHookServerUrl(),
  });
}

