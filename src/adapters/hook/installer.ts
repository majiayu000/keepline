/**
 * Claude hooks installer
 *
 * Registers a Keepline hook under Claude Code's real settings shape:
 *   hooks.<Event>[] = [{ matcher?, hooks: [{ type: "command", command }] }]
 *
 * The command forwards Claude's stdin hook JSON verbatim to the hook server;
 * it does NOT rely on `$CLAUDE_*` environment variables (those do not exist —
 * Claude Code delivers hook data on stdin only).
 */

import { existsSync, readFileSync, writeFileSync } from 'fs';
import { CLAUDE_SETTINGS } from '../../lib/paths.js';
import { config } from '../../lib/config.js';
import { logger } from '../../lib/logger.js';
import type {
  ClaudeSettings,
  ClaudeHookMatcher,
  HookCommandEntry,
  HookEventType,
} from './types.js';

const KEEPLINE_HOOK_MARKER = 'KEEPLINE_HOOK_MARKER=keepline-hook-v2';

/** Event types Keepline registers a forwarding hook under */
const KEEPLINE_EVENTS: HookEventType[] = ['PostToolUse', 'Stop', 'UserPromptSubmit'];

/** Every event type we scan when uninstalling (covers historical registrations) */
const ALL_HOOK_EVENTS: HookEventType[] = [
  'PreToolUse',
  'PostToolUse',
  'Notification',
  'Stop',
  'UserPromptSubmit',
];

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

/**
 * Generate the hook command.
 *
 * `--data-binary @-` forwards Claude's stdin JSON verbatim (no newline
 * stripping, no urlencoding). The marker prefix is a harmless env assignment
 * used only to identify Keepline-owned hooks in the settings file.
 */
function getHookCommand(): string {
  const port = config.get().hookPort;
  return `${KEEPLINE_HOOK_MARKER} curl -s -X POST http://127.0.0.1:${port}/hook -H 'Content-Type: application/json' --data-binary @- >/dev/null 2>&1 || true`;
}

/** Detect the pre-v2 command that (incorrectly) relied on `$CLAUDE_*` env vars */
function isLegacyKeeplineHookCommand(command: string): boolean {
  return (
    command.includes('curl -s -X POST') &&
    /\bhttp:\/\/127\.0\.0\.1:\d+\/hook\b/.test(command) &&
    command.includes('"event_type":"$CLAUDE_EVENT_TYPE"') &&
    command.includes('"session_id":"$CLAUDE_SESSION_ID"')
  );
}

/** Is this command string a Keepline-owned hook (any version)? */
export function isKeeplineHookCommand(command: string): boolean {
  return command.includes('KEEPLINE_HOOK_MARKER=') || isLegacyKeeplineHookCommand(command);
}

/** Is this settings hook entry a Keepline-owned command hook? */
export function isKeeplineHook(hook: unknown): hook is HookCommandEntry {
  if (!hook || typeof hook !== 'object') {
    return false;
  }
  const command = (hook as Partial<HookCommandEntry>).command;
  return typeof command === 'string' && isKeeplineHookCommand(command);
}

/** Find the Keepline-owned command entry among a list of matcher blocks */
function findKeeplineEntry(blocks: ClaudeHookMatcher[]): HookCommandEntry | undefined {
  for (const block of blocks) {
    const entry = block.hooks?.find(isKeeplineHook);
    if (entry) return entry;
  }
  return undefined;
}

/**
 * Ensure a Keepline hook is registered under every KEEPLINE_EVENTS type.
 * Returns true if the settings object was modified.
 */
export function installKeeplineHookConfig(
  settings: ClaudeSettings,
  hookCommand: string = getHookCommand()
): boolean {
  if (!settings.hooks) {
    settings.hooks = {};
  }

  let changed = false;

  for (const event of KEEPLINE_EVENTS) {
    const blocks = settings.hooks[event] ?? (settings.hooks[event] = []);
    const existing = findKeeplineEntry(blocks);

    if (existing) {
      // Upgrade an older/legacy Keepline hook in place, without duplicating.
      if (existing.command !== hookCommand || existing.type !== 'command') {
        existing.command = hookCommand;
        existing.type = 'command';
        changed = true;
      }
    } else {
      blocks.push({ hooks: [{ type: 'command', command: hookCommand }] });
      changed = true;
    }
  }

  return changed;
}

/**
 * Remove every Keepline-owned command hook, preserving unrelated hooks and
 * matcher blocks. Blocks that become empty (they were Keepline-only) are
 * dropped. Returns the number of command hooks removed.
 */
export function uninstallKeeplineHookConfig(settings: ClaudeSettings): number {
  if (!settings.hooks) return 0;

  let removed = 0;

  for (const event of ALL_HOOK_EVENTS) {
    const blocks = settings.hooks[event];
    if (!blocks) continue;

    const nextBlocks: ClaudeHookMatcher[] = [];
    for (const block of blocks) {
      const originalHooks = block.hooks ?? [];
      const keptHooks = originalHooks.filter((hook) => !isKeeplineHook(hook));
      removed += originalHooks.length - keptHooks.length;

      // Keep blocks that still hold unrelated hooks; drop Keepline-only blocks.
      if (keptHooks.length > 0) {
        nextBlocks.push({ ...block, hooks: keptHooks });
      }
    }

    settings.hooks[event] = nextBlocks;
  }

  return removed;
}

/** Are Keepline hooks installed under any registered event? */
function hasKeeplineHook(settings: ClaudeSettings): boolean {
  return KEEPLINE_EVENTS.some((event) => {
    const blocks = settings.hooks?.[event];
    return Boolean(blocks && findKeeplineEntry(blocks));
  });
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
