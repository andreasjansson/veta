#!/usr/bin/env node
// This script starts wrangler dev and keeps running until killed
// It's designed to be run as a background process that doesn't die on SIGHUP

import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));

// Ignore SIGHUP in this process
process.on('SIGHUP', () => {
  console.log('Received SIGHUP, ignoring...');
});

const wrangler = spawn('npx', ['wrangler', 'dev', '--port', '8787'], {
  cwd: __dirname,
  stdio: 'inherit',
  detached: true,
});

wrangler.unref();

wrangler.on('exit', (code, signal) => {
  console.log(`Wrangler exited with code ${code}, signal ${signal}`);
  process.exit(code || 0);
});

// Keep the script alive
setInterval(() => {}, 1000000);

// Forward SIGTERM and SIGINT to wrangler
process.on('SIGTERM', () => {
  process.kill(-wrangler.pid, 'SIGTERM');
  process.exit(0);
});
process.on('SIGINT', () => {
  process.kill(-wrangler.pid, 'SIGINT');
  process.exit(0);
});
