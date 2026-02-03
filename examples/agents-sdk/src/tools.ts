import { tool, type ToolSet } from "ai";
import { z } from "zod/v3";
import { getCurrentAgent } from "agents";
import type { Chat } from "./agent";

function getVetaFetcher() {
  const ctx = getCurrentAgent<Chat>();
  if (!ctx?.agent) {
    throw new Error("No agent context available");
  }
  // @ts-expect-error - env is protected but accessible at runtime
  const fetcher = ctx.agent.env.VETA as Fetcher;
  if (!fetcher) {
    throw new Error("VETA service binding not available");
  }
  return fetcher;
}

const VetaCommand = z.discriminatedUnion("command", [
  z.object({
    command: z.literal("add").describe("Add a new note"),
    title: z.string().describe("Note title"),
    body: z.string().describe("Note content"),
    tags: z.array(z.string()).describe("Tags for organization"),
  }),
  z.object({
    command: z.literal("ls").describe("List notes, optionally filtered by tags"),
    tags: z.array(z.string()).optional().describe("Filter by these tags"),
  }),
  z.object({
    command: z.literal("show").describe("Show the full content of a specific note"),
    id: z.number().describe("Note ID"),
  }),
  z.object({
    command: z.literal("grep").describe("Search notes by pattern (regex supported)"),
    query: z.string().describe("Search pattern"),
    tags: z.array(z.string()).optional().describe("Filter by these tags"),
  }),
  z.object({
    command: z.literal("tags").describe("List all tags"),
  }),
  z.object({
    command: z.literal("rm").describe("Delete one or more notes"),
    ids: z.array(z.number()).describe("Note IDs to delete"),
  }),
]);

type VetaInput = z.infer<typeof VetaCommand>;

async function executeVetaCommand(input: VetaInput): Promise<string> {
  const veta = getVetaFetcher();

  switch (input.command) {
    case "add": {
      const res = await veta.fetch("http://veta/notes", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          title: input.title,
          body: input.body,
          tags: input.tags,
        }),
      });
      if (!res.ok) {
        const error = await res.text();
        return `Error adding note: ${error}`;
      }
      const data = (await res.json()) as { id: number };
      return `Added note ${data.id}: ${input.title}`;
    }

    case "ls": {
      const params = input.tags?.length ? `?tags=${input.tags.join(",")}` : "";
      const res = await veta.fetch(`http://veta/notes${params}`);
      const notes = (await res.json()) as {
        id: number;
        title: string;
        body_preview: string;
        tags: string[];
      }[];
      if (!notes.length) return "No notes found.";
      return notes
        .map((n) => `[${n.id}] ${n.title} -- ${n.body_preview}`)
        .join("\n");
    }

    case "show": {
      const res = await veta.fetch(`http://veta/notes/${input.id}`);
      if (!res.ok) return `Note ${input.id} not found.`;
      const note = (await res.json()) as {
        title: string;
        body: string;
        tags: string[];
      };
      return `# ${note.title}\n\n${note.body}\n\nTags: ${note.tags.join(", ")}`;
    }

    case "grep": {
      const params = new URLSearchParams({ q: input.query });
      if (input.tags?.length) params.set("tags", input.tags.join(","));
      const res = await veta.fetch(`http://veta/grep?${params}`);
      if (!res.ok) {
        const error = await res.text();
        return `Search error: ${error}`;
      }
      const notes = (await res.json()) as {
        id: number;
        title: string;
        body_preview: string;
        tags: string[];
      }[];
      if (!notes.length) return "No matching notes found.";
      return notes
        .map((n) => `[${n.id}] ${n.title} -- ${n.body_preview}`)
        .join("\n");
    }

    case "tags": {
      const res = await veta.fetch("http://veta/tags");
      const tags = (await res.json()) as { name: string; count: number }[];
      if (!tags.length) return "No tags found.";
      return tags.map((t) => `${t.name} (${t.count} notes)`).join("\n");
    }

    case "rm": {
      const results: string[] = [];
      for (const id of input.ids) {
        const res = await veta.fetch(`http://veta/notes/${id}`, {
          method: "DELETE",
        });
        results.push(res.ok ? `Deleted note ${id}.` : `Note ${id} not found.`);
      }
      return results.join("\n");
    }
  }
}

export const tools = {
  veta: tool({
    description:
      "Interact with the Veta knowledge base. Commands: add (create note), ls (list notes), show (view note), grep (search), tags (list tags), rm (delete notes)",
    inputSchema: VetaCommand,
    execute: executeVetaCommand,
  }),
} satisfies ToolSet;
