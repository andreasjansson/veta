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
- At conversation start, check listNotes to recall context
- Before answering questions, search your memory - you may already know the answer
- Use searchNotes when the user asks about something you might have stored

WRITING MEMORY - save notes when you:
- Learn user preferences (communication style, interests, opinions)
- Discover facts about the user (name, location, job, relationships)
- Are told something worth remembering ("my favorite X is Y", "I work at Z")
- Make decisions or have insights worth preserving
- Learn something the user might ask about later

GOOD NOTES have:
- Searchable titles ("User's favorite color" not "Note about colors")
- Relevant tags for organization (preferences, facts, interests)
- Concise but complete information

Don't wait to be asked to remember - if it seems worth knowing later, save it now.`,
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
