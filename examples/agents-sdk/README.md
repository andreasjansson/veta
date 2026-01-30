# Veta Agents SDK Example

A minimal example showing how to build a Cloudflare Agent with Veta as persistent memory.

## Components

- **Agent Worker** (`src/agent.ts`) - Chat agent using the Cloudflare Agents SDK with Veta tools
- **Veta Worker** (`src/veta.ts`) - Re-exports the Veta npm package as a service binding
- **D1 Database** - Stores Veta notes (auto-provisioned on deploy)
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

1. Set your OpenAI API key as a secret:

```bash
npx wrangler secret put OPENAI_API_KEY
```

2. Deploy both workers:

```bash
npm run deploy
```

The D1 database is auto-provisioned on first deploy, and migrations are run automatically.

This deploys the Veta worker first (since the agent depends on it), then the agent worker.

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
