import { routeAgentRequest } from "agents";
import { AIChatAgent } from "@cloudflare/ai-chat";
import {
  streamText,
  createUIMessageStream,
  convertToModelMessages,
  createUIMessageStreamResponse,
  stepCountIs,
  type StreamTextOnFinishCallback,
  type ToolSet,
} from "ai";
import { createOpenAI } from "@ai-sdk/openai";
import { tools } from "./tools";

export class Chat extends AIChatAgent<Env> {
  async onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>,
    options?: { abortSignal?: AbortSignal }
  ) {
    const apiKey = this.env.OPENAI_API_KEY;

    if (!apiKey) {
      const errorMessage =
        "OPENAI_API_KEY is not set. " +
        "Run: npx wrangler secret put OPENAI_API_KEY -c wrangler.agent.jsonc -- " +
        "Or add it to .dev.vars for local development.";

      const stream = createUIMessageStream({
        execute: async ({ writer }) => {
          writer.write({ type: "error", errorText: errorMessage });
        },
      });
      return createUIMessageStreamResponse({ stream });
    }

    const openai = createOpenAI({ apiKey });
    const model = openai("gpt-4o-mini");

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        const result = streamText({
          system: `You are a helpful assistant with persistent memory via Veta.

YOUR MEMORY PERSISTS ACROSS CONVERSATIONS. Use it proactively:

READING MEMORY:
- At conversation start, use veta(command: "ls") to recall context
- Before answering questions, search your memory with veta(command: "grep") - you may already know the answer
- Use grep when the user asks about something you might have stored

WRITING MEMORY - save notes when you learn or discovered something that you don't already know, e.g.:
- Specifics about a task you're working on
- Discoveries you make as you're working on a task
- User preferences and facts about the user
- Something the user explictly tells you to remember
- Decisions and the reasoning behind them
- Something the user might ask about later

GOOD NOTES have:
- Searchable titles ("User's favorite color" not "Note about colors")
- Relevant tags for organization (preferences, facts, interests)
- Concise but complete information
- References when applicable

Don't wait to be asked to remember - if it seems worth knowing later, save it now.

VETA COMMANDS:
- veta(command: "add", title, body, tags) - Add a new note
- veta(command: "ls", tags?) - List notes, optionally filtered by tags
- veta(command: "show", id) - Show full content of a note
- veta(command: "grep", query, tags?) - Search notes by pattern
- veta(command: "tags") - List all tags
- veta(command: "rm", ids) - Delete notes`,
          messages: await convertToModelMessages(this.messages),
          model,
          tools,
          onFinish: onFinish as unknown as StreamTextOnFinishCallback<typeof tools>,
          stopWhen: stepCountIs(5),
          abortSignal: options?.abortSignal,
        });
        writer.merge(result.toUIMessageStream());
      },
    });
    return createUIMessageStreamResponse({ stream });
  }
}

export default {
  async fetch(request: Request, env: Env, _ctx: ExecutionContext) {
    const url = new URL(request.url);

    if (url.pathname === "/check-open-ai-key") {
      return Response.json({ success: !!env.OPENAI_API_KEY });
    }

    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
