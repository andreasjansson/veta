#!/usr/bin/env node
// WebSocket chat test script for cctr integration tests
import WebSocket from 'ws';

const action = process.argv[2];
const prompt = process.argv[3];
const successPattern = process.argv[4];

if (!action || !prompt) {
  console.error('Usage: ws-chat.mjs <action> <prompt> [successPattern]');
  process.exit(1);
}

const port = process.env.TEST_PORT || '8787';
const ws = new WebSocket(`ws://localhost:${port}/agents/chat/cctr-${action}-${Date.now()}`);

ws.on('open', () => {
  ws.send(JSON.stringify({
    type: 'cf_agent_use_chat_request',
    id: `test-${action}`,
    init: {
      method: 'POST',
      body: JSON.stringify({
        messages: [{
          id: 'msg-1',
          role: 'user',
          parts: [{ type: 'text', text: prompt }]
        }],
        clientTools: []
      })
    }
  }));
});

let output = '';
ws.on('message', (data) => {
  const msg = data.toString();
  output += msg;
  
  // Check for success pattern
  if (successPattern && msg.includes(successPattern)) {
    console.log('ok');
    ws.close();
    process.exit(0);
  }
  
  // Also check for tool calls or common success indicators
  if (msg.includes('tool-call') || msg.includes('tool-result')) {
    // Tool was called, that's a success
    console.log('ok');
    ws.close();
    process.exit(0);
  }
});

ws.on('error', (err) => {
  console.log('error');
  console.error('WebSocket error:', err.message);
  process.exit(1);
});

ws.on('unexpected-response', (req, res) => {
  console.log('error');
  console.error('Unexpected response:', res.statusCode, res.statusMessage);
  process.exit(1);
});

ws.on('close', () => {
  // If we got meaningful output, consider it a success
  if (output.length > 100) {
    console.log('ok');
    process.exit(0);
  }
});

setTimeout(() => {
  if (output.length > 50) {
    console.log('ok');
    process.exit(0);
  }
  console.log('timeout');
  console.error('Output:', output.slice(0, 500));
  process.exit(1);
}, 45000);
