/**
 * Codex session data types.
 */

import type { ParsedSessionData } from '../../domain/session/index.js';

export interface CodexSessionFile {
  sessionId: string;
  rawSessionId: string;
  directory: string;
  filePath: string;
  modifiedAt: Date;
}

export type CodexParsedSessionData = ParsedSessionData & {
  client: 'codex';
  rawSessionId: string;
};

export interface CodexJsonlEntry {
  type: 'session_meta' | 'turn_context' | 'response_item' | 'event_msg' | string;
  timestamp?: string;
  payload?: Record<string, unknown>;
}
