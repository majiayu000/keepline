/**
 * Web command - start the web UI server
 */

import { startWebServer } from '../web/api/server.js';
import { config, isValidPortNumber } from '../lib/config.js';

interface WebOptions {
  port?: string;
}

export async function webCommand(options: WebOptions): Promise<void> {
  let port: number;
  try {
    port = resolveWebPort(options.port);
  } catch {
    console.error('Invalid port number');
    process.exit(1);
  }

  const server = await startWebServer(port);

  const shutdown = () => {
    server.stop();
    process.exit(0);
  };

  process.once('SIGINT', shutdown);
  process.once('SIGTERM', shutdown);

  await new Promise<void>(() => {});
}

export function resolveWebPort(portOption?: string): number {
  const trimmed = portOption?.trim();
  if (!trimmed) return config.get().webPort;

  const port = Number(trimmed);
  if (!isValidPortNumber(port)) {
    throw new Error('Invalid port number');
  }

  return port;
}
