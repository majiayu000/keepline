/**
 * Claude hooks installer
 */

import { existsSync, readFileSync, writeFileSync } from 'fs';
import { CLAUDE_SETTINGS } from '../../lib/paths.js';
import { config } from '../../lib/config.js';
import { logger } from '../../lib/logger.js';
import type { ClaudeSettings, ClaudeHookConfig } from './types.js';

const KEEPLINE_HOOK_MARKER = 'KEEPLINE_HOOK_MARKER=keepline-hook-v1';
const HOOK_TYPES = ['PreToolUse', 'PostToolUse', 'Notification', 'Stop', 'UserPromptSubmit'] as const;

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
  return `${KEEPLINE_HOOK_MARKER} curl -s -X POST http://127.0.0.1:${port}/hook -H "Content-Type: application/json" -d '{"event_type":"$CLAUDE_EVENT_TYPE","session_id":"$CLAUDE_SESSION_ID","cwd":"$PWD","timestamp":"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'","tool_name":"$CLAUDE_TOOL_NAME","tool_input":'"\${CLAUDE_TOOL_INPUT:-{}}"'}' > /dev/null 2>&1 || true`;
}

function isLegacyKeeplineHookCommand(command: string): boolean {
  return (
    command.includes('curl -s -X POST') &&
    /\bhttp:\/\/127\.0\.0\.1:\d+\/hook\b/.test(command) &&
    command.includes('"event_type":"$CLAUDE_EVENT_TYPE"') &&
    command.includes('"session_id":"$CLAUDE_SESSION_ID"') &&
    command.includes('"cwd":"$PWD"') &&
    command.includes('"tool_name":"$CLAUDE_TOOL_NAME"') &&
    command.includes('CLAUDE_TOOL_INPUT')
  );
}

export function isKeeplineHook(hook: unknown): hook is ClaudeHookConfig {
  if (
    !hook ||
    typeof hook !== 'object' ||
    typeof (hook as Partial<ClaudeHookConfig>).command !== 'string'
  ) {
    return false;
  }
  const command = (hook as ClaudeHookConfig).command;
  return command.includes(KEEPLINE_HOOK_MARKER) || isLegacyKeeplineHookCommand(command);
}

export function installKeeplineHookConfig(
  settings: ClaudeSettings,
  hookCommand: string = getHookCommand()
): boolean {
  if (!settings.hooks) {
    settings.hooks = {};
  }

  if (!settings.hooks.PostToolUse) {
    settings.hooks.PostToolUse = [];
  }

  const existingIndex = settings.hooks.PostToolUse.findIndex(isKeeplineHook);
  const keeplineHook: ClaudeHookConfig = {
    command: hookCommand,
  };

  if (existingIndex >= 0) {
    const existing = settings.hooks.PostToolUse[existingIndex];
    if (existing.command !== hookCommand) {
      settings.hooks.PostToolUse[existingIndex] = {
        ...existing,
        command: hookCommand,
      };
      return true;
    }
    return false;
  }

  settings.hooks.PostToolUse.push(keeplineHook);
  return true;
}

export function uninstallKeeplineHookConfig(settings: ClaudeSettings): number {
  if (!settings.hooks) return 0;

  let removed = 0;
  for (const hookType of HOOK_TYPES) {
    const hooks = settings.hooks[hookType];
    if (!hooks) continue;
    const nextHooks = hooks.filter((hook) => !isKeeplineHook(hook));
    removed += hooks.length - nextHooks.length;
    settings.hooks[hookType] = nextHooks;
  }

  return removed;
}

function hasKeeplineHook(settings: ClaudeSettings): boolean {
  return Boolean(
    settings.hooks?.PostToolUse?.some(isKeeplineHook)
  );
}

/** Check if keepline hooks are installed */
export function areHooksInstalled(): boolean {
  const settings = getClaudeSettings();
  return hasKeeplineHook(settings);
}

/** Install keepline hooks into Claude settings */
export function installHooks(): void {
  const settings = getClaudeSettings();
  if (installKeeplineHookConfig(settings)) {
    saveClaudeSettings(settings);
    logger.info('Keepline hooks installed');
  } else {
    logger.debug('Keepline hooks already installed');
  }
}

/** Uninstall keepline hooks from Claude settings */
export function uninstallHooks(): void {
  const settings = getClaudeSettings();

  const removed = uninstallKeeplineHookConfig(settings);
  if (removed > 0) {
    saveClaudeSettings(settings);
    logger.info('Keepline hooks uninstalled');
  } else {
    logger.debug('Keepline hooks not installed');
  }
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
