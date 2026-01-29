#!/usr/bin/env node
// Start the multi-worker dev server for testing

import { spawn, execSync } from 'child_process';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const exampleDir = join(__dirname, '..', '..', '..', 'examples', 'agent-sdk');

process.on('SIGHUP', () => {
  console.log('Received SIGHUP, ignoring...');
});

// Find npx in the example directory's node_modules
const npxPath = join(exampleDir, 'node_modules', '.bin', 'wrangler');

const wrangler = spawn(npxPath, [
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
