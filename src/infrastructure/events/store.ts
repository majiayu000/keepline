/**
 * Event store - persists events to SQLite
 */

import type { DomainEvent } from '../../domain/shared/types.js';
import { getDatabase, execSql, queryAll, runSql, type SQLBindParam } from '../database/sqlite.js';
import { logger } from '../../lib/logger.js';

/** Stored event record */
export interface StoredEvent {
  id: number;
  type: string;
  aggregateId: string | null;
  payload: string;
  timestamp: string;
}

/** Ensure events table exists */
function ensureEventsTable(): void {
  execSql(`
    CREATE TABLE IF NOT EXISTS events (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      type TEXT NOT NULL,
      aggregate_id TEXT,
      payload TEXT,
      timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    )
  `);

  execSql(`
    CREATE INDEX IF NOT EXISTS idx_events_type ON events(type);
    CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_id);
    CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
  `);
}

/** Event store class */
export class EventStore {
  private initialized = false;

  private ensureInitialized(): void {
    if (!this.initialized) {
      ensureEventsTable();
      this.initialized = true;
    }
  }

  /**
   * Append an event to the store
   */
  async append(event: DomainEvent): Promise<number> {
    this.ensureInitialized();

    const payload = JSON.stringify(event.payload || {});

    runSql(
      `INSERT INTO events (type, aggregate_id, payload, timestamp) VALUES (?, ?, ?, ?)`,
      [event.type, event.aggregateId || null, payload, event.timestamp.toISOString()]
    );

    // Get the last inserted ID
    const db = getDatabase();
    const result = db.prepare('SELECT last_insert_rowid() as id').get() as { id: number };

    logger.debug(`Event stored: ${event.type}`, { id: result.id });
    return result.id;
  }

  /**
   * Get events by type
   */
  async getByType(type: string, limit = 100): Promise<DomainEvent[]> {
    this.ensureInitialized();

    const rows = queryAll<StoredEvent>(
      `SELECT * FROM events WHERE type = ? ORDER BY timestamp DESC LIMIT ?`,
      [type, limit]
    );

    return rows.map(this.rowToEvent);
  }

  /**
   * Get events by aggregate ID
   */
  async getByAggregateId(aggregateId: string): Promise<DomainEvent[]> {
    this.ensureInitialized();

    const rows = queryAll<StoredEvent>(
      `SELECT * FROM events WHERE aggregate_id = ? ORDER BY timestamp ASC`,
      [aggregateId]
    );

    return rows.map(this.rowToEvent);
  }

  /**
   * Get events since a timestamp
   */
  async getSince(since: Date, limit = 1000): Promise<DomainEvent[]> {
    this.ensureInitialized();

    const rows = queryAll<StoredEvent>(
      `SELECT * FROM events WHERE timestamp >= ? ORDER BY timestamp ASC LIMIT ?`,
      [since.toISOString(), limit]
    );

    return rows.map(this.rowToEvent);
  }

  /**
   * Get all events (with pagination)
   */
  async getAll(limit = 100, offset = 0): Promise<DomainEvent[]> {
    this.ensureInitialized();

    const rows = queryAll<StoredEvent>(
      `SELECT * FROM events ORDER BY timestamp DESC LIMIT ? OFFSET ?`,
      [limit, offset]
    );

    return rows.map(this.rowToEvent);
  }

  /**
   * Count events
   */
  async count(type?: string): Promise<number> {
    this.ensureInitialized();

    const db = getDatabase();
    let sql = 'SELECT COUNT(*) as count FROM events';
    const params: SQLBindParam[] = [];

    if (type) {
      sql += ' WHERE type = ?';
      params.push(type);
    }

    const result = db.prepare(sql).get(...params) as { count: number };
    return result.count;
  }

  /**
   * Delete old events
   */
  async deleteOlderThan(date: Date): Promise<number> {
    this.ensureInitialized();

    const db = getDatabase();
    const countBefore = (
      db.prepare('SELECT COUNT(*) as count FROM events WHERE timestamp < ?').get(date.toISOString()) as { count: number }
    ).count;

    runSql('DELETE FROM events WHERE timestamp < ?', [date.toISOString()]);

    logger.info(`Deleted ${countBefore} old events`);
    return countBefore;
  }

  /**
   * Convert database row to DomainEvent
   */
  private rowToEvent(row: StoredEvent): DomainEvent {
    return {
      type: row.type,
      aggregateId: row.aggregateId || undefined,
      payload: JSON.parse(row.payload || '{}'),
      timestamp: new Date(row.timestamp),
    };
  }
}

/** Singleton instance */
export const eventStore = new EventStore();
