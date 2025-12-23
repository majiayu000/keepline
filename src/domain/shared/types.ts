/**
 * Shared domain types
 */

/** Base entity interface */
export interface Entity {
  id: string;
  createdAt: Date;
  updatedAt: Date;
}

/** Result type for operations that can fail */
export type Result<T, E = Error> =
  | { success: true; data: T }
  | { success: false; error: E };

/** Optional type wrapper */
export type Option<T> = T | null;

/** Domain event base interface */
export interface DomainEvent {
  readonly type: string;
  readonly timestamp: Date;
  readonly aggregateId?: string;
  readonly payload?: Record<string, unknown>;
}

/** Event handler interface */
export interface EventHandler<T extends DomainEvent = DomainEvent> {
  handle(event: T): Promise<void>;
}

/** Repository base interface */
export interface Repository<T extends Entity> {
  findById(id: string): T | null;
  findAll(): T[];
  save(entity: Partial<T> & { id?: string }): T;
  delete(id: string): boolean;
}
