#!/usr/bin/env node
// Send a message to the agent and print the response
import WebSocket from 'ws';

const message = process.argv[2];
if (!message) {
  console.error('Usage: test-ws-chat.mjs <message>');
  process.exit(1);
}

const port = process.env.TEST_PORT || '8787';
const agentId = process.env.TEST_AGENT_ID || `test-${Date.now()}`;
const ws = new WebSocket(`ws://localhost:${port}/agents/chat/${agentId}`);

ws.on('open', () => {
  ws.send(JSON.stringify({
    type: 'cf_agent_use_chat_request',
    id: 'test-1',
    init: {
      method: 'POST',
      body: JSON.stringify({
        messages: [{
          id: 'msg-1',
          role: 'user',
          parts: [{ type: 'text', text: message }]
        }],
        clientTools: []
      })
    }
  }));
});

let output = '';
let done = false;

ws.on('message', (data) => {
  const msg = data.toString();
  
  // Skip the initial MCP message
  if (msg.includes('cf_agent_mcp_servers')) return;
  
  try {
    const parsed = JSON.parse(msg);
    if (parsed.type === 'cf_agent_use_chat_response') {
      // Extract the body content
      if (parsed.body) {
        try {
          const body = JSON.parse(parsed.body);
          // Collect text deltas and tool outputs
          if (body.type === 'text-delta' && body.textDelta) {
            output += body.textDelta;
          } else if (body.type === 'tool-output-available' && body.output) {
            output += body.output + '\n';
          }
        } catch (e) {
          // Body wasn't JSON, append as-is
          output += parsed.body;
        }
      }
      // Check if this is the final message
      if (parsed.done) {
        done = true;
        ws.close();
      }
    }
  } catch (e) {
    // Not JSON, ignore
  }
});

ws.on('close', () => {
  console.log(output.trim());
  process.exit(0);
});

ws.on('error', (err) => {
  console.error('WebSocket error:', err.message);
  process.exit(1);
});

// Timeout after 60 seconds
setTimeout(() => {
  if (!done) {
    console.error('Timeout waiting for response');
    console.log(output.trim());
    process.exit(1);
  }
}, 60000);
