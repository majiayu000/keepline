/**
 * Hook module types
 */

/** Hook event types from Claude Code */
export type HookEventType =
  | 'PreToolUse'
  | 'PostToolUse'
  | 'Notification'
  | 'Stop'
  | 'UserPromptSubmit'; // User submitted a prompt

/** Base hook event payload */
export interface HookEventPayload {
  event_type: HookEventType;
  session_id: string;
  cwd: string;
  timestamp: string;
  transcript_path?: string;
}

/** Tool use hook event */
export interface ToolUseHookEvent extends HookEventPayload {
  event_type: 'PreToolUse' | 'PostToolUse';
  tool_name: string;
  tool_input: Record<string, unknown>;
  tool_output?: string;
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
  is_first_prompt?: boolean;
}

/** Union type for all hook events */
export type HookEvent =
  | ToolUseHookEvent
  | NotificationHookEvent
  | StopHookEvent
  | UserPromptSubmitHookEvent;

/** Claude settings hook configuration */
export interface ClaudeHookCommandHandler {
  type?: 'command' | string;
  command: string;
  args?: string[];
  timeout?: number;
  [key: string]: unknown;
}

export interface ClaudeHookMatcherGroup {
  matcher?: string;
  hooks: ClaudeHookCommandHandler[];
  [key: string]: unknown;
}

export type ClaudeHookConfig = ClaudeHookCommandHandler | ClaudeHookMatcherGroup;

/** Claude settings structure */
export interface ClaudeSettings {
  hooks?: {
    PreToolUse?: ClaudeHookConfig[];
    PostToolUse?: ClaudeHookConfig[];
    Notification?: ClaudeHookConfig[];
    Stop?: ClaudeHookConfig[];
    UserPromptSubmit?: ClaudeHookConfig[];
  };
  [key: string]: unknown;
}
