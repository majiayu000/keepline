/**
 * Recovery module types
 */

/** Recovery method options */
export type RecoveryMethod =
  | 'resume'    // Use claude --resume <sessionId>
  | 'continue'  // Use claude --continue
  | 'new';      // Start new session with same prompt

/** Recovery options */
export interface RecoveryOptions {
  method: RecoveryMethod;
  sessionId: string;
  directory: string;
  openTerminal?: boolean;
  skipPermissions?: boolean;
  terminalApp?: TerminalApp;
}

/** Recovery result */
export interface RecoveryResult {
  success: boolean;
  method: RecoveryMethod;
  sessionId: string;
  pid?: number;
  error?: string;
}

/** Terminal app options */
export type TerminalApp = 'Terminal' | 'iTerm' | 'Warp' | 'auto';

/** Available terminal apps list */
export const TERMINAL_APPS = ['Terminal', 'iTerm', 'Warp'] as const;
