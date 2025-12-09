/**
 * Custom error classes for Tasker
 */

/** Base error class */
export class TaskerError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly details?: Record<string, unknown>
  ) {
    super(message);
    this.name = 'TaskerError';
    Error.captureStackTrace(this, this.constructor);
  }
}

/** Database related errors */
export class DatabaseError extends TaskerError {
  constructor(message: string, details?: Record<string, unknown>) {
    super(message, 'DB_ERROR', details);
    this.name = 'DatabaseError';
  }
}

/** Session not found error */
export class SessionNotFoundError extends TaskerError {
  constructor(sessionId: string) {
    super(`Session not found: ${sessionId}`, 'SESSION_NOT_FOUND', { sessionId });
    this.name = 'SessionNotFoundError';
  }
}

/** Parse error for JSONL files */
export class ParseError extends TaskerError {
  constructor(filePath: string, line: number, originalError?: Error) {
    super(
      `Failed to parse ${filePath} at line ${line}`,
      'PARSE_ERROR',
      { filePath, line, originalError: originalError?.message }
    );
    this.name = 'ParseError';
  }
}

/** Process scan error */
export class ProcessScanError extends TaskerError {
  constructor(message: string, details?: Record<string, unknown>) {
    super(message, 'PROCESS_SCAN_ERROR', details);
    this.name = 'ProcessScanError';
  }
}

/** Recovery error */
export class RecoveryError extends TaskerError {
  constructor(sessionId: string, reason: string) {
    super(`Failed to recover session ${sessionId}: ${reason}`, 'RECOVERY_ERROR', { sessionId, reason });
    this.name = 'RecoveryError';
  }
}

/** Configuration error */
export class ConfigError extends TaskerError {
  constructor(message: string) {
    super(message, 'CONFIG_ERROR');
    this.name = 'ConfigError';
  }
}
