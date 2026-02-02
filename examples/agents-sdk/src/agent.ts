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
          system: `You are a helpful assistant with access to a persistent memory/notes database called Veta.

IMPORTANT MEMORY RULES:
1. When the user asks you to remember something, ALWAYS use addNote to save it.
2. When the user asks ANY question (especially about preferences, facts, or things you might have been told before), FIRST use listNotes or searchNotes to check your memory before answering.
3. If this is the start of a conversation and the user asks a question, check Veta first - you may have relevant notes from previous sessions.
4. When saving notes, use descriptive titles and relevant tags for easy retrieval.

Your memory persists across conversations. Always check it when the user asks about something that could have been stored previously.`,
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
