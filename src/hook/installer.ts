/**
 * Claude hooks installer
 */

import { existsSync, readFileSync, writeFileSync } from 'fs';
import { CLAUDE_SETTINGS } from '../utils/paths.js';
import { config } from '../utils/config.js';
import { logger } from '../utils/logger.js';
import type { ClaudeSettings, ClaudeHookConfig } from './types.js';

/** Get current Claude settings */
function getClaudeSettings(): ClaudeSettings {
  if (!existsSync(CLAUDE_SETTINGS)) {
    logger.debug('Claude settings file not found, using defaults');
    return {};
  }

  try {
    const content = readFileSync(CLAUDE_SETTINGS, 'utf-8');
    return JSON.parse(content) as ClaudeSettings;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    logger.warn(`Failed to parse Claude settings: ${message}`, { path: CLAUDE_SETTINGS });
    // Return empty settings to allow operation to continue
    return {};
  }
}

/** Save Claude settings */
function saveClaudeSettings(settings: ClaudeSettings): void {
  writeFileSync(CLAUDE_SETTINGS, JSON.stringify(settings, null, 2));
}

/** Generate hook command */
function getHookCommand(): string {
  const port = config.get().hookPort;
  return `curl -s -X POST http://127.0.0.1:${port}/hook -H "Content-Type: application/json" -d '{"event_type":"$CLAUDE_EVENT_TYPE","session_id":"$CLAUDE_SESSION_ID","cwd":"$PWD","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'","tool_name":"$CLAUDE_TOOL_NAME","tool_input":'"\${CLAUDE_TOOL_INPUT:-{}}"'}' > /dev/null 2>&1 || true`;
}

/** Check if tasker hooks are installed */
export function areHooksInstalled(): boolean {
  const settings = getClaudeSettings();
  const hooks = settings.hooks;

  if (!hooks) return false;

  // Check if our hook is in PostToolUse
  const postToolUse = hooks.PostToolUse || [];
  return postToolUse.some((hook) => hook.command.includes('127.0.0.1'));
}

/** Install tasker hooks into Claude settings */
export function installHooks(): void {
  const settings = getClaudeSettings();
  const hookCommand = getHookCommand();

  const taskerHook: ClaudeHookConfig = {
    command: hookCommand,
  };

  // Initialize hooks if not exists
  if (!settings.hooks) {
    settings.hooks = {};
  }

  // Add to PostToolUse
  if (!settings.hooks.PostToolUse) {
    settings.hooks.PostToolUse = [];
  }

  // Check if already installed
  const existing = settings.hooks.PostToolUse.find((h) =>
    h.command.includes('127.0.0.1')
  );

  if (!existing) {
    settings.hooks.PostToolUse.push(taskerHook);
    saveClaudeSettings(settings);
    logger.info('Tasker hooks installed');
  } else {
    logger.debug('Tasker hooks already installed');
  }
}

/** Uninstall tasker hooks from Claude settings */
export function uninstallHooks(): void {
  const settings = getClaudeSettings();

  if (!settings.hooks) return;

  // Remove from all hook types
  const hookTypes = ['PreToolUse', 'PostToolUse', 'Notification', 'Stop'] as const;

  for (const hookType of hookTypes) {
    const hooks = settings.hooks[hookType];
    if (hooks) {
      settings.hooks[hookType] = hooks.filter(
        (h) => !h.command.includes('127.0.0.1')
      );
    }
  }

  saveClaudeSettings(settings);
  logger.info('Tasker hooks uninstalled');
}

/** Get hook status info */
export function getHookStatus(): {
  installed: boolean;
  settingsPath: string;
  hookCommand: string;
} {
  return {
    installed: areHooksInstalled(),
    settingsPath: CLAUDE_SETTINGS,
    hookCommand: getHookCommand(),
  };
}
