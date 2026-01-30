#!/usr/bin/env node
// Start the multi-worker dev server for testing

import { spawn } from 'child_process';
import { join } from 'path';

const exampleDir = process.env.VETA_EXAMPLE_DIR;
if (!exampleDir) {
  console.error('VETA_EXAMPLE_DIR not set');
  process.exit(1);
}

process.on('SIGHUP', () => {
  console.log('Received SIGHUP, ignoring...');
});

// Use wrangler directly from node_modules
const wranglerPath = join(exampleDir, 'node_modules', '.bin', 'wrangler');

const wrangler = spawn(wranglerPath, [
  'dev',
  '-c', 'wrangler.agent.jsonc',
  '-c', 'wrangler.veta.jsonc',
  '--port', '8788'
], {
  cwd: exampleDir,
  stdio: 'inherit',
  detached: true,
});

wrangler.unref();

wrangler.on('exit', (code, signal) => {
  console.log(`Wrangler exited with code ${code}, signal ${signal}`);
  process.exit(code || 0);
});

setInterval(() => {}, 1000000);

process.on('SIGTERM', () => {
  process.kill(-wrangler.pid, 'SIGTERM');
  process.exit(0);
});
process.on('SIGINT', () => {
  process.kill(-wrangler.pid, 'SIGINT');
  process.exit(0);
});
