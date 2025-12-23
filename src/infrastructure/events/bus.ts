/**
 * Enhanced event bus with persistence support
 */

import EventEmitter from 'eventemitter3';
import type { DomainEvent, EventHandler } from '../../domain/shared/types.js';
import { logger } from '../../lib/logger.js';

/** Event bus configuration */
export interface EventBusConfig {
  /** Enable event persistence */
  persist?: boolean;
  /** Maximum number of handlers per event */
  maxHandlers?: number;
}

/** Event bus class */
export class EventBus {
  private emitter: EventEmitter;
  private handlers: Map<string, EventHandler[]> = new Map();
  private config: Required<EventBusConfig>;

  constructor(config: EventBusConfig = {}) {
    this.emitter = new EventEmitter();
    this.config = {
      persist: config.persist ?? false,
      maxHandlers: config.maxHandlers ?? 100,
    };
  }

  /**
   * Subscribe to an event type
   */
  subscribe<T extends DomainEvent>(
    eventType: string,
    handler: EventHandler<T>
  ): () => void {
    const handlers = this.handlers.get(eventType) || [];

    if (handlers.length >= this.config.maxHandlers) {
      logger.warn(`Max handlers reached for event: ${eventType}`);
    }

    handlers.push(handler as EventHandler);
    this.handlers.set(eventType, handlers);

    // Also register with EventEmitter for compatibility
    this.emitter.on(eventType, handler.handle.bind(handler));

    // Return unsubscribe function
    return () => {
      const currentHandlers = this.handlers.get(eventType) || [];
      const index = currentHandlers.indexOf(handler as EventHandler);
      if (index > -1) {
        currentHandlers.splice(index, 1);
        this.handlers.set(eventType, currentHandlers);
      }
      this.emitter.off(eventType, handler.handle.bind(handler));
    };
  }

  /**
   * Subscribe using a simple function
   */
  on<T extends DomainEvent>(
    eventType: string,
    handler: (event: T) => void | Promise<void>
  ): () => void {
    const wrappedHandler: EventHandler<T> = {
      handle: async (event: T) => {
        await handler(event);
      },
    };
    return this.subscribe(eventType, wrappedHandler);
  }

  /**
   * Publish an event
   */
  async publish<T extends DomainEvent>(event: T): Promise<void> {
    const handlers = this.handlers.get(event.type) || [];

    logger.debug(`Publishing event: ${event.type}`, {
      aggregateId: event.aggregateId,
      handlerCount: handlers.length,
    });

    // Execute all handlers in parallel, but don't fail if one fails
    const results = await Promise.allSettled(
      handlers.map((handler) =>
        handler.handle(event).catch((err) => {
          logger.error(`Event handler failed for ${event.type}`, {
            error: err instanceof Error ? err.message : String(err),
          });
          throw err;
        })
      )
    );

    // Log any failures
    const failures = results.filter((r) => r.status === 'rejected');
    if (failures.length > 0) {
      logger.warn(`${failures.length}/${handlers.length} handlers failed for ${event.type}`);
    }

    // Also emit via EventEmitter for compatibility
    this.emitter.emit(event.type, event);
  }

  /**
   * Emit event (alias for publish, for EventEmitter compatibility)
   */
  emit<T extends DomainEvent>(event: T): void {
    // Fire and forget
    this.publish(event).catch((err) => {
      logger.error('Failed to publish event', { error: err });
    });
  }

  /**
   * Get handler count for an event type
   */
  listenerCount(eventType: string): number {
    return this.handlers.get(eventType)?.length || 0;
  }

  /**
   * Remove all handlers for an event type
   */
  removeAllListeners(eventType?: string): void {
    if (eventType) {
      this.handlers.delete(eventType);
      this.emitter.removeAllListeners(eventType);
    } else {
      this.handlers.clear();
      this.emitter.removeAllListeners();
    }
  }
}

/** Global event bus instance */
let globalEventBus: EventBus | null = null;

/** Get or create global event bus */
export function getEventBus(): EventBus {
  if (!globalEventBus) {
    globalEventBus = new EventBus();
  }
  return globalEventBus;
}

/** Reset global event bus (for testing) */
export function resetEventBus(): void {
  if (globalEventBus) {
    globalEventBus.removeAllListeners();
  }
  globalEventBus = null;
}
