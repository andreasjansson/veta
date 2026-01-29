# Veta Agent SDK Example

A minimal example showing how to build a Cloudflare Agent with Veta as persistent memory.

## Components

- **Agent Worker** (`src/agent.ts`) - Chat agent using the Cloudflare Agents SDK with Veta tools
- **Veta Worker** (`src/veta.ts`) - Re-exports the Veta npm package as a service binding
- **UI** (`public/index.html`) - Minimal React+Tailwind chat interface

## Local Development

1. Install dependencies:

```bash
npm install
```

2. Create a D1 database:

```bash
npx wrangler d1 create veta-example-db
```

3. Copy the `database_id` from the output and update `wrangler.veta.jsonc`.

4. Run migrations (migrations are in `node_modules/veta/migrations/`):

```bash
npx wrangler d1 migrations apply veta-example-db --local
```

5. Create `.dev.vars` with your OpenAI API key:

```bash
cp .dev.vars.example .dev.vars
# Edit .dev.vars and add your OPENAI_API_KEY
```

6. Start the dev server:

```bash
npm run dev
```

Open http://localhost:8787

## Deployment

1. Run remote migrations:

```bash
npx wrangler d1 migrations apply veta-example-db --remote
```

2. Set your OpenAI API key as a secret:

```bash
npx wrangler secret put OPENAI_API_KEY
```

3. Deploy both workers:

```bash
npm run deploy
```

This deploys the Veta worker first (since the agent depends on it), then the agent worker.

## How It Works

The agent has access to these Veta tools:

- `addNote` - Store information with tags
- `listNotes` - Browse notes, optionally filtered by tag
- `showNote` - Read a specific note
- `searchNotes` - Search with regex patterns
- `listTags` - See all available tags
- `deleteNote` - Remove a note

The Veta worker runs as a separate service, and the agent calls it via service bindings (the `VETA` binding in `wrangler.jsonc`).

## Example Prompts

- "Remember that the project uses Tailwind v4"
- "What do you remember about this project?"
- "Search my notes for anything about authentication"
- "Show me note 1"
- "List all my tags"
