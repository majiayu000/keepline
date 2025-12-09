/**
 * Event system for Tasker
 */

import EventEmitter from 'eventemitter3';
import type { SessionEventPayload, ToolEventPayload } from './types.js';

/** Event types */
export type TaskerEvents = {
  // Session events
  'session:discovered': SessionEventPayload;
  'session:updated': SessionEventPayload;
  'session:lost': SessionEventPayload;
  'session:recovered': SessionEventPayload;
  'session:completed': SessionEventPayload;

  // Tool events (from hooks)
  'tool:pre': ToolEventPayload;
  'tool:post': ToolEventPayload;

  // System events
  'daemon:started': { pid: number };
  'daemon:stopped': { reason: string };
  'scan:complete': { sessionCount: number; duration: number };
  'error': { error: Error; context: string };
};

/** Global event emitter instance */
class TaskerEventEmitter extends EventEmitter<TaskerEvents> {
  private static instance: TaskerEventEmitter;

  private constructor() {
    super();
  }

  static getInstance(): TaskerEventEmitter {
    if (!TaskerEventEmitter.instance) {
      TaskerEventEmitter.instance = new TaskerEventEmitter();
    }
    return TaskerEventEmitter.instance;
  }
}

/** Export singleton instance */
export const events = TaskerEventEmitter.getInstance();

/** Helper to emit typed events */
export function emit<K extends keyof TaskerEvents>(
  event: K,
  payload: TaskerEvents[K]
): void {
  events.emit(event, payload);
}

/** Helper to listen to typed events */
export function on<K extends keyof TaskerEvents>(
  event: K,
  handler: (payload: TaskerEvents[K]) => void
): void {
  events.on(event, handler as (...args: unknown[]) => void);
}
