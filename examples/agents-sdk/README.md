# Veta Agents SDK Example

A minimal example showing how to build a Cloudflare Agent with Veta as persistent memory.

## Components

- **Agent Worker** (`src/agent.ts`) - Chat agent using the Cloudflare Agents SDK with Veta tools
- **Veta Worker** (`src/veta.ts`) - Re-exports the Veta npm package as a service binding
- **D1 Database** - Stores Veta notes (configured in `wrangler.veta.jsonc`)
- **UI** (`public/index.html`) - Minimal React+Tailwind chat interface

## Local Development

1. Install dependencies:

```bash
npm install
```

2. Run migrations locally:

```bash
npx wrangler d1 migrations apply VETA_DB -c wrangler.veta.jsonc --local
```

3. Create `.dev.vars` with your OpenAI API key:

```bash
cp .dev.vars.example .dev.vars
# Edit .dev.vars and add your OPENAI_API_KEY
```

4. Start the dev server:

```bash
npm run dev
```

Open http://localhost:8787

## Deployment

1. Create a D1 database:

```bash
npx wrangler d1 create veta-example-db
```

2. Copy the `database_id` from the output and update `wrangler.veta.jsonc`.

3. Set your OpenAI API key as a secret (`.dev.vars` is only for local dev):

```bash
npx wrangler secret put OPENAI_API_KEY -c wrangler.agent.jsonc
```

4. Deploy both workers (migrations run automatically):

```bash
npm run deploy
```

This deploys the Veta worker first (with migrations), then the agent worker.

## How It Works

The agent has access to these Veta tools:

- `addNote` - Store information with tags
- `listNotes` - Browse notes, optionally filtered by tag
- `showNote` - Read a specific note
- `searchNotes` - Search with regex patterns
- `listTags` - See all available tags
- `rmNotes` - Remove one or more notes

The Veta worker runs as a separate service, and the agent calls it via service bindings (the `VETA` binding in `wrangler.agent.jsonc`).

## Example Prompts

- "Remember that the project uses Tailwind v4"
- "What do you remember about this project?"
- "Search my notes for anything about authentication"
- "Show me note 1"
- "List all my tags"
