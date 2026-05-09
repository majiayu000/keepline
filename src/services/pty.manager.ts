/**
 * PTY Session Manager
 *
 * Manages pseudo-terminal sessions with scrollback buffer,
 * detach/reattach support, and lifecycle tracking.
 * Uses bun-pty (native Bun PTY binding) instead of node-pty.
 */

import { randomUUID } from 'crypto';
import { spawn as spawnPty } from 'bun-pty';
import { runSql } from '../infrastructure/database/sqlite.js';
import { config } from '../lib/config.js';
import { logger } from '../lib/logger.js';

interface IPty {
  pid: number;
  cols: number;
  rows: number;
  onData: (callback: (data: string) => void) => { dispose: () => void };
  onExit: (callback: (e: { exitCode: number; signal?: string }) => void) => { dispose: () => void };
  write: (data: string) => void;
  resize: (cols: number, rows: number) => void;
  kill: (signal?: string) => void;
}

interface ServerWebSocket {
  send: (data: string | Buffer) => void;
}

export interface PtySession {
  id: string;
  userId: string;
  pty: IPty;
  scrollback: string[];
  scrollbackBytes: number;
  attachedClients: Set<ServerWebSocket>;
  status: 'running' | 'exited';
  exitCode?: number;
  cwd: string;
  createdAt: Date;
  lastActivity: Date;
}

export interface SessionInfo {
  id: string;
  pid: number;
  cwd: string;
  status: string;
  exitCode?: number;
  createdAt: string;
  clientCount: number;
}

class PtyManager {
  sessions = new Map<string, PtySession>();
  private idleTimer: ReturnType<typeof setInterval> | null = null;

  constructor() {
    // Start idle cleanup timer
    this.idleTimer = setInterval(() => this.cleanupIdle(), 60_000);
  }

  async create(userId: string, cols: number = 80, rows: number = 24, cwd?: string, resumeSessionId?: string): Promise<PtySession> {
    const cfg = config.get().webTerminal;

    // Check max sessions
    const userSessions = [...this.sessions.values()].filter(
      s => s.userId === userId && s.status === 'running'
    );
    if (userSessions.length >= cfg.maxSessions) {
      throw new Error(`Max sessions (${cfg.maxSessions}) reached`);
    }

    const sessionId = randomUUID();
    const spawnCwd = cwd || process.env.HOME || '/';

    // Build command: if resumeSessionId provided, use `claude --resume <id>`
    let file: string;
    let args: string[];
    if (resumeSessionId) {
      file = 'claude';
      args = ['--resume', resumeSessionId];
    } else {
      const shellCmd = cfg.shellCommand || 'claude';
      const shellParts = shellCmd.split(' ');
      file = shellParts[0];
      args = shellParts.slice(1);
    }

    const pty = spawnPty(file, args, {
      name: 'xterm-256color',
      cols,
      rows,
      cwd: spawnCwd,
      env: {
        HOME: process.env.HOME || '',
        PATH: process.env.PATH || '',
        TERM: 'xterm-256color',
        LANG: process.env.LANG || 'en_US.UTF-8',
        USER: process.env.USER || '',
        SHELL: process.env.SHELL || '/bin/bash',
        COLORTERM: 'truecolor',
      },
    }) as unknown as IPty;

    const session: PtySession = {
      id: sessionId,
      userId,
      pty,
      scrollback: [],
      scrollbackBytes: 0,
      attachedClients: new Set(),
      status: 'running',
      cwd: spawnCwd,
      createdAt: new Date(),
      lastActivity: new Date(),
    };

    // Capture output -> scrollback + broadcast to attached clients
    pty.onData((data: string) => {
      session.lastActivity = new Date();
      this.appendScrollback(session, data);
      const msg = JSON.stringify({ type: 'term:output', data: { sessionId, data } });
      for (const client of session.attachedClients) {
        try {
          client.send(msg);
        } catch (e) {
          logger.debug(`PTY ${sessionId}: client send failed (likely disconnected)`, e);
        }
      }
    });

    // Handle exit
    pty.onExit(({ exitCode }) => {
      session.status = 'exited';
      session.exitCode = exitCode;
      const msg = JSON.stringify({ type: 'term:exited', data: { sessionId, exitCode } });
      for (const client of session.attachedClients) {
        try {
          client.send(msg);
        } catch (e) {
          logger.debug(`PTY ${sessionId}: client send (exit) failed`, e);
        }
      }
      // Update DB
      runSql(
        'UPDATE terminal_sessions SET status = ?, exit_code = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?',
        ['exited', exitCode, sessionId]
      );
      logger.info(`PTY session ${sessionId} exited with code ${exitCode}`);
    });

    this.sessions.set(sessionId, session);

    // Record in DB
    runSql(
      'INSERT INTO terminal_sessions (id, user_id, pid, cwd, status) VALUES (?, ?, ?, ?, ?)',
      [sessionId, userId, pty.pid, spawnCwd, 'running']
    );

    logger.info(`PTY session ${sessionId} created (pid: ${pty.pid})`);
    return session;
  }

  private getOwnedSession(sessionId: string, userId: string): PtySession {
    const session = this.sessions.get(sessionId);
    if (!session) throw new Error('Session not found');
    if (session.userId !== userId) throw new Error('Forbidden');
    return session;
  }

  attach(sessionId: string, userId: string, ws: ServerWebSocket): PtySession {
    const session = this.getOwnedSession(sessionId, userId);

    // Replay scrollback
    if (session.scrollback.length > 0) {
      const scrollbackData = session.scrollback.join('');
      ws.send(JSON.stringify({ type: 'term:scrollback', data: { sessionId, data: scrollbackData } }));
    }

    session.attachedClients.add(ws);
    logger.info(`Client attached to session ${sessionId} (${session.attachedClients.size} clients)`);
    return session;
  }

  detach(sessionId: string, userId: string, ws: ServerWebSocket): void {
    const session = this.sessions.get(sessionId);
    if (session) {
      if (session.userId !== userId) {
        throw new Error('Forbidden');
      }
      session.attachedClients.delete(ws);
      logger.info(`Client detached from session ${sessionId} (${session.attachedClients.size} clients)`);
    }
  }

  write(sessionId: string, userId: string, data: string): void {
    const session = this.getOwnedSession(sessionId, userId);
    if (session.status !== 'running') return;
    session.pty.write(data);
    session.lastActivity = new Date();
  }

  resize(sessionId: string, userId: string, cols: number, rows: number): void {
    const session = this.getOwnedSession(sessionId, userId);
    if (session.status !== 'running') return;
    session.pty.resize(cols, rows);
  }

  kill(sessionId: string, userId: string): void {
    const session = this.getOwnedSession(sessionId, userId);
    if (session.status !== 'running') return;
    session.pty.kill();
    logger.info(`PTY session ${sessionId} killed`);
  }

  listSessions(userId?: string): SessionInfo[] {
    const result: SessionInfo[] = [];
    for (const session of this.sessions.values()) {
      if (userId && session.userId !== userId) continue;
      result.push({
        id: session.id,
        pid: session.pty.pid,
        cwd: session.cwd,
        status: session.status,
        exitCode: session.exitCode,
        createdAt: session.createdAt.toISOString(),
        clientCount: session.attachedClients.size,
      });
    }
    return result;
  }

  detachAll(ws: ServerWebSocket): void {
    for (const session of this.sessions.values()) {
      session.attachedClients.delete(ws);
    }
  }

  private appendScrollback(session: PtySession, data: string): void {
    const maxSize = config.get().webTerminal.scrollbackSize;
    session.scrollback.push(data);
    session.scrollbackBytes += data.length;

    // Trim if over limit
    while (session.scrollbackBytes > maxSize && session.scrollback.length > 1) {
      const removed = session.scrollback.shift()!;
      session.scrollbackBytes -= removed.length;
    }
  }

  private cleanupIdle(): void {
    const timeoutMin = config.get().webTerminal.idleTimeoutMinutes;
    if (timeoutMin <= 0) return;

    const cutoff = Date.now() - timeoutMin * 60_000;
    for (const session of this.sessions.values()) {
      if (session.status === 'running' && session.lastActivity.getTime() < cutoff && session.attachedClients.size === 0) {
        logger.info(`Killing idle session ${session.id}`);
        session.pty.kill();
      }
    }
  }

  cleanup(): void {
    if (this.idleTimer) clearInterval(this.idleTimer);
    for (const session of this.sessions.values()) {
      if (session.status === 'running') {
        session.pty.kill();
      }
    }
    this.sessions.clear();
  }
}

export const ptyManager = new PtyManager();
