import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { ptyManager } from '../services/pty.manager.js';
import { config } from '../lib/config.js';
import { resetDatabase } from '../db/migrations.js';
import { closeDatabase, queryOne } from '../infrastructure/database/sqlite.js';
import { setupUser } from '../services/auth.service.js';

const shellCommand = process.env.SHELL || '/bin/sh';
const mockClient = { send() {} };

describe('PTY session ownership', () => {
  const originalTerminalConfig = { ...config.get().webTerminal };
  let ownerId = '';

  beforeEach(async () => {
    resetDatabase();
    config.set('webTerminal', {
      ...config.get().webTerminal,
      shellCommand,
    });

    await setupUser('pty-owner', 'password123');
    ownerId = queryOne<{ id: string }>(
      'SELECT id FROM terminal_users WHERE username = ?',
      ['pty-owner']
    )!.id;
  });

  afterEach(() => {
    ptyManager.cleanup();
    config.set('webTerminal', originalTerminalConfig);
    closeDatabase();
  });

  test('rejects attach for a different user', async () => {
    const session = await ptyManager.create(ownerId, 80, 24, process.cwd());
    expect(() => ptyManager.attach(session.id, 'owner-b', mockClient)).toThrow('Forbidden');
  });

  test('rejects write and kill for a different user', async () => {
    const session = await ptyManager.create(ownerId, 80, 24, process.cwd());
    expect(() => ptyManager.write(session.id, 'owner-b', 'pwd\n')).toThrow('Forbidden');
    expect(() => ptyManager.kill(session.id, 'owner-b')).toThrow('Forbidden');
  });
});
