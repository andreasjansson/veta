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
    // @ts-expect-error - env is protected but we need it for the API key
    const apiKey = this.env.OPENAI_API_KEY;
    
    const openai = createOpenAI({ apiKey });
    const model = openai("gpt-4o-mini");

    const stream = createUIMessageStream({
      execute: async ({ writer }) => {
        const result = streamText({
          system: `You are a helpful assistant with access to a memory/notes database called Veta.
Use Veta to remember important information across conversations.`,
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
