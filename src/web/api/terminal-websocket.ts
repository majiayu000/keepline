/**
 * Terminal WebSocket Handler
 *
 * Handles /ws/terminal connections with auth-first protocol.
 * Dispatches term:* messages to PtyManager.
 */

import type { ServerWebSocket } from 'bun';
import { verifyToken } from '../../services/auth.service.js';
import { ptyManager } from '../../services/pty.manager.js';
import { logger } from '../../lib/logger.js';

interface TerminalWsState {
  authenticated: boolean;
  userId?: string;
  username?: string;
}

const clientState = new WeakMap<ServerWebSocket<any>, TerminalWsState>();

function send(ws: ServerWebSocket<any>, msg: object) {
  try {
    ws.send(JSON.stringify(msg));
  } catch (e) {
    logger.debug('Terminal WS send failed (client likely disconnected)', e);
  }
}

export const terminalWebsocketHandler = {
  open(ws: ServerWebSocket<any>) {
    clientState.set(ws, { authenticated: false });
    logger.info('Terminal WebSocket client connected');
  },

  message(ws: ServerWebSocket<any>, message: string | Buffer) {
    let msg: any;
    try {
      msg = JSON.parse(message.toString());
    } catch {
      send(ws, { type: 'error', data: { message: 'Invalid JSON' } });
      return;
    }

    const state = clientState.get(ws);
    if (!state) return;

    // Auth-first: only accept auth message until authenticated
    if (!state.authenticated) {
      if (msg.type !== 'auth') {
        send(ws, { type: 'auth:error', data: { message: 'Not authenticated' } });
        return;
      }
      handleAuth(ws, state, msg.data);
      return;
    }

    // Dispatch authenticated messages
    switch (msg.type) {
      case 'term:create':
        handleCreate(ws, state, msg.data);
        break;
      case 'term:attach':
        handleAttach(ws, state, msg.data);
        break;
      case 'term:input':
        handleInput(ws, state, msg.data);
        break;
      case 'term:resize':
        handleResize(ws, state, msg.data);
        break;
      case 'term:detach':
        handleDetach(ws, state, msg.data);
        break;
      case 'term:kill':
        handleKill(ws, state, msg.data);
        break;
      case 'term:list':
        handleList(ws, state);
        break;
      case 'ping':
        send(ws, { type: 'pong' });
        break;
      default:
        send(ws, { type: 'error', data: { message: `Unknown type: ${msg.type}` } });
    }
  },

  close(ws: ServerWebSocket<any>) {
    ptyManager.detachAll(ws as any);
    clientState.delete(ws);
    logger.info('Terminal WebSocket client disconnected');
  },
};

function handleAuth(ws: ServerWebSocket<any>, state: TerminalWsState, data: any) {
  if (!data?.token) {
    send(ws, { type: 'auth:error', data: { message: 'Token required' } });
    return;
  }

  const payload = verifyToken(data.token);
  if (!payload) {
    send(ws, { type: 'auth:error', data: { message: 'Invalid token' } });
    return;
  }

  state.authenticated = true;
  state.userId = payload.sub;
  state.username = payload.username;
  send(ws, { type: 'auth:ok' });
  logger.info(`Terminal WS authenticated: ${payload.username}`);
}

async function handleCreate(ws: ServerWebSocket<any>, state: TerminalWsState, data: any) {
  try {
    const session = await ptyManager.create(
      state.userId!,
      data?.cols || 80,
      data?.rows || 24,
      data?.cwd,
      data?.resumeSessionId
    );
    // Auto-attach creator
    ptyManager.attach(session.id, state.userId!, ws as any);
    send(ws, { type: 'term:created', data: { sessionId: session.id, pid: session.pty.pid } });
  } catch (e) {
    send(ws, { type: 'error', data: { message: (e as Error).message } });
  }
}

function handleAttach(ws: ServerWebSocket<any>, state: TerminalWsState, data: any) {
  try {
    const session = ptyManager.attach(data.sessionId, state.userId!, ws as any);
    send(ws, { type: 'term:attached', data: { sessionId: session.id, status: session.status } });
  } catch (e) {
    send(ws, { type: 'error', data: { message: (e as Error).message } });
  }
}

function handleInput(ws: ServerWebSocket<any>, state: TerminalWsState, data: any) {
  if (data?.sessionId && data?.data) {
    try {
      ptyManager.write(data.sessionId, state.userId!, data.data);
    } catch (e) {
      send(ws, { type: 'error', data: { message: (e as Error).message } });
    }
  }
}

function handleResize(ws: ServerWebSocket<any>, state: TerminalWsState, data: any) {
  if (data?.sessionId && data?.cols && data?.rows) {
    try {
      ptyManager.resize(data.sessionId, state.userId!, data.cols, data.rows);
    } catch (e) {
      send(ws, { type: 'error', data: { message: (e as Error).message } });
    }
  }
}

function handleDetach(ws: ServerWebSocket<any>, state: TerminalWsState, data: any) {
  if (data?.sessionId) {
    try {
      ptyManager.detach(data.sessionId, state.userId!, ws as any);
    } catch (e) {
      send(ws, { type: 'error', data: { message: (e as Error).message } });
    }
  }
}

function handleKill(ws: ServerWebSocket<any>, state: TerminalWsState, data: any) {
  if (data?.sessionId) {
    try {
      ptyManager.kill(data.sessionId, state.userId!);
    } catch (e) {
      send(ws, { type: 'error', data: { message: (e as Error).message } });
    }
  }
}

function handleList(ws: ServerWebSocket<any>, state: TerminalWsState) {
  const sessions = ptyManager.listSessions(state.userId);
  send(ws, { type: 'term:list', data: { sessions } });
}
