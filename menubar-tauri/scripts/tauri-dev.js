#!/usr/bin/env node
/**
 * Smart Tauri Dev Launcher
 * - Auto-detects available port
 * - Passes config to Tauri via --config flag (no file modification!)
 * - Sets environment variable for Vite
 */
import { detect } from 'detect-port';
import { spawn } from 'child_process';

const BASE_PORT = 1420;

async function main() {
  const port = await detect(BASE_PORT);

  if (port !== BASE_PORT) {
    console.log(`\x1b[33m[Auto-Port]\x1b[0m Port ${BASE_PORT} occupied, using \x1b[32m${port}\x1b[0m`);
  } else {
    console.log(`\x1b[32m[Auto-Port]\x1b[0m Using port ${port}`);
  }

  // Build config override JSON
  const configOverride = JSON.stringify({
    build: {
      devUrl: `http://localhost:${port}`
    }
  });

  // Start Tauri with config override
  const child = spawn('npx', [
    'tauri', 'dev',
    '--config', configOverride
  ], {
    stdio: 'inherit',
    env: {
      ...process.env,
      VITE_DEV_PORT: String(port)
    }
  });

  child.on('exit', (code) => process.exit(code || 0));
}

main();
