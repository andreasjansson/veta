# Veta Agents SDK Example

A minimal example showing how to build a Cloudflare Agent with Veta as persistent memory.

## Components

- **Agent Worker** (`src/agent.ts`) - Chat agent using the Cloudflare Agents SDK with Veta tools
- **Veta Worker** (`src/veta.ts`) - Re-exports the Veta npm package as a service binding
- **D1 Database** - Stores Veta notes (auto-provisioned and auto-migrated)
- **UI** (`public/index.html`) - Minimal React+Tailwind chat interface

## Local Development

1. Install dependencies:

```bash
npm install
```

2. Create `.dev.vars` with your OpenAI API key:

```bash
cp .dev.vars.example .dev.vars
# Edit .dev.vars and add your OPENAI_API_KEY
```

3. Start the dev server:

```bash
npm run dev
```

Open http://localhost:8787

The D1 database is created automatically on first request, and migrations run automatically.

## Deployment

1. Set your OpenAI API key as a secret:

```bash
npx wrangler secret put OPENAI_API_KEY -c wrangler.agent.jsonc
```

2. Deploy both workers:

```bash
npm run deploy
```

The D1 database is auto-provisioned on first deploy, and migrations run automatically on first request.

## Cleanup

To completely remove all deployed resources (workers, D1 database, Durable Objects):

```bash
npm run undeploy
```

## How It Works

The agent has access to a unified `veta` tool with these commands:

- `veta add` - Store information with tags
- `veta ls` - Browse notes, optionally filtered by tag
- `veta show` - Read a specific note
- `veta grep` - Search with regex patterns
- `veta tags` - See all available tags
- `veta rm` - Remove one or more notes

The Veta worker runs as a separate service, and the agent calls it via service bindings (the `VETA` binding in `wrangler.agent.jsonc`).

## Example Prompts

- "Remember that the project uses Tailwind v4"
- "What do you remember about this project?"
- "Search my notes for anything about authentication"
- "Show me note 1"
- "List all my tags"
