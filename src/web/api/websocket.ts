/**
 * WebSocket Management
 *
 * Handles WebSocket client connections and broadcasting
 */

import type { ServerWebSocket } from 'bun';
import { logger } from '../../lib/logger.js';

// WebSocket client management
export interface WebSocketClient {
  ws: ServerWebSocket<unknown>;
  subscribedTo: Set<string>; // session IDs or 'all'
}

export const wsClients = new Set<WebSocketClient>();

// Broadcast message to all connected clients
export function broadcast(type: string, data: unknown) {
  const message = JSON.stringify({ type, data, timestamp: new Date().toISOString() });
  for (const client of wsClients) {
    try {
      client.ws.send(message);
    } catch (error) {
      logger.error('Failed to send WebSocket message', error);
    }
  }
}

// Broadcast to clients subscribed to a specific session
export function broadcastToSession(sessionId: string, type: string, data: unknown) {
  const message = JSON.stringify({ type, data, sessionId, timestamp: new Date().toISOString() });
  for (const client of wsClients) {
    if (client.subscribedTo.has('all') || client.subscribedTo.has(sessionId)) {
      try {
        client.ws.send(message);
      } catch (error) {
        logger.error('Failed to send WebSocket message', error);
      }
    }
  }
}

// WebSocket handler for Bun.serve
export const websocketHandler = {
  open(ws: ServerWebSocket<unknown>) {
    const client: WebSocketClient = {
      ws,
      subscribedTo: new Set(['all']),
    };
    wsClients.add(client);
    (ws as any).__client = client;
    logger.info(`WebSocket client connected (${wsClients.size} total)`);

    // Send initial data
    ws.send(JSON.stringify({
      type: 'connected',
      data: { clientCount: wsClients.size },
      timestamp: new Date().toISOString(),
    }));
  },
  message(ws: ServerWebSocket<unknown>, message: string | Buffer) {
    try {
      const data = JSON.parse(message.toString());
      const client = (ws as any).__client as WebSocketClient;

      // Handle subscription messages
      if (data.type === 'subscribe') {
        if (data.sessionId) {
          client.subscribedTo.add(data.sessionId);
        }
      } else if (data.type === 'unsubscribe') {
        if (data.sessionId) {
          client.subscribedTo.delete(data.sessionId);
        }
      } else if (data.type === 'ping') {
        ws.send(JSON.stringify({ type: 'pong', timestamp: new Date().toISOString() }));
      }
    } catch (error) {
      logger.error('Failed to parse WebSocket message', error);
    }
  },
  close(ws: ServerWebSocket<unknown>) {
    const client = (ws as any).__client as WebSocketClient;
    if (client) {
      wsClients.delete(client);
    }
    logger.info(`WebSocket client disconnected (${wsClients.size} remaining)`);
  },
};
