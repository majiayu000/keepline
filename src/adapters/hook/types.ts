/**
 * Hook module types
 */

/** Hook event types from Claude Code that Keepline consumes */
export type HookEventType =
  | 'PreToolUse'
  | 'PostToolUse'
  | 'Notification'
  | 'Stop'
  | 'UserPromptSubmit'; // User submitted a prompt

/**
 * Internal, normalized hook event payload.
 *
 * Claude Code delivers hook data as JSON on stdin (fields such as
 * `hook_event_name`, `tool_response`) and does NOT set `$CLAUDE_*` env vars.
 * The server parses that native payload into these normalized shapes; the
 * `timestamp` is stamped on receipt because Claude does not send one.
 */
export interface HookEventPayload {
  session_id: string;
  cwd: string;
  timestamp: string;
}

/** Tool use hook event */
export interface ToolUseHookEvent extends HookEventPayload {
  event_type: 'PreToolUse' | 'PostToolUse';
  tool_name: string;
  tool_input: Record<string, unknown>;
  tool_output?: string; // Only present in PostToolUse
}

/** Notification hook event */
export interface NotificationHookEvent extends HookEventPayload {
  event_type: 'Notification';
  message: string;
}

/** Stop hook event */
export interface StopHookEvent extends HookEventPayload {
  event_type: 'Stop';
  reason?: string;
}

/** User prompt submit event */
export interface UserPromptSubmitHookEvent extends HookEventPayload {
  event_type: 'UserPromptSubmit';
  prompt: string;
}

/** Union type for all hook events */
export type HookEvent =
  | ToolUseHookEvent
  | NotificationHookEvent
  | StopHookEvent
  | UserPromptSubmitHookEvent;

/**
 * A single command entry inside a Claude settings hook matcher block.
 * This is Claude Code's real nested shape: `hooks[].hooks[]`.
 */
export interface HookCommandEntry {
  type: 'command';
  command: string;
  timeout?: number;
}

/** A matcher block: optionally filters events, and runs one or more commands */
export interface ClaudeHookMatcher {
  matcher?: string;
  hooks: HookCommandEntry[];
}

/** Claude settings structure (hooks use the nested matcher-block shape) */
export interface ClaudeSettings {
  hooks?: Partial<Record<HookEventType, ClaudeHookMatcher[]>>;
  [key: string]: unknown;
}
