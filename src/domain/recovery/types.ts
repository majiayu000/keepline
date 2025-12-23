/**
 * Recovery domain types
 */

/** Recovery method options */
export type RecoveryMethod =
  | 'resume'    // Use claude --resume <sessionId>
  | 'continue'  // Use claude --continue
  | 'new';      // Start new session with same prompt

/** Terminal app options */
export type TerminalApp = 'Terminal' | 'iTerm' | 'Warp' | 'auto';

/** Available terminal apps list */
export const TERMINAL_APPS = ['Terminal', 'iTerm', 'Warp'] as const;

/** Recovery options */
export interface RecoveryOptions {
  method: RecoveryMethod;
  sessionId: string;
  directory: string;
  openTerminal?: boolean;
  skipPermissions?: boolean;
  terminalApp?: TerminalApp;
  /** [NEW] Inject context from memory system */
  injectContext?: boolean;
}

/** Recovery result */
export interface RecoveryResult {
  success: boolean;
  method: RecoveryMethod;
  sessionId: string;
  pid?: number;
  error?: string;
  /** [NEW] Context that was injected */
  contextInjected?: boolean;
}

/** Recovery status check result */
export interface RecoveryStatus {
  canRecover: boolean;
  reason?: string;
  availableMethods: RecoveryMethod[];
}

/** Recovery info with recommendation */
export interface RecoveryInfo extends RecoveryStatus {
  recommendedMethod?: RecoveryMethod;
  command?: string;
}
