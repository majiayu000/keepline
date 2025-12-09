/**
 * Hook module types
 */

/** Hook event types from Claude Code */
export type HookEventType =
  | 'PreToolUse'
  | 'PostToolUse'
  | 'Notification'
  | 'Stop';

/** Base hook event payload */
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

/** Union type for all hook events */
export type HookEvent =
  | ToolUseHookEvent
  | NotificationHookEvent
  | StopHookEvent;

/** Claude settings hook configuration */
export interface ClaudeHookConfig {
  matcher?: string;
  command: string;
}

/** Claude settings structure */
export interface ClaudeSettings {
  hooks?: {
    PreToolUse?: ClaudeHookConfig[];
    PostToolUse?: ClaudeHookConfig[];
    Notification?: ClaudeHookConfig[];
    Stop?: ClaudeHookConfig[];
  };
  [key: string]: unknown;
}
