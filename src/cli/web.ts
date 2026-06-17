/**
 * Web command - start the web UI server
 */

import { startWebServer } from '../web/api/server.js';

interface WebOptions {
  port?: string;
}

export async function webCommand(options: WebOptions): Promise<void> {
  const port = options.port ? parseInt(options.port, 10) : 3377;

  if (isNaN(port) || port < 1 || port > 65535) {
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
