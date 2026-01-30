import { tool, type ToolSet } from "ai";
import { z } from "zod/v3";
import { getCurrentAgent } from "agents";
import type { Chat } from "./agent";

function veta() {
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

export const tools = {
  addNote: tool({
    description: "Add a note to the Veta knowledge base",
    inputSchema: z.object({
      title: z.string().describe("Note title"),
      body: z.string().describe("Note content"),
      tags: z.array(z.string()).describe("Tags for organization"),
    }),
    execute: async ({ title, body, tags }) => {
      const res = await veta().fetch("http://veta/notes", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ title, body, tags }),
      });
      if (!res.ok) {
        const error = await res.text();
        return `Error adding note: ${error}`;
      }
      const data = (await res.json()) as { id: number };
      return `Added note ${data.id}: ${title}`;
    },
  }),

  listNotes: tool({
    description: "List notes from the Veta knowledge base, optionally filtered by tags",
    inputSchema: z.object({
      tags: z.array(z.string()).optional().describe("Filter by these tags"),
    }),
    execute: async ({ tags }) => {
      const params = tags?.length ? `?tags=${tags.join(",")}` : "";
      const res = await veta().fetch(`http://veta/notes${params}`);
      const notes = (await res.json()) as { id: number; title: string; body_preview: string; tags: string[] }[];
      if (!notes.length) return "No notes found.";
      return notes.map((n) => `[${n.id}] ${n.title} (${n.tags.join(", ")})\n${n.body_preview}`).join("\n\n");
    },
  }),

  showNote: tool({
    description: "Show the full content of a specific note",
    inputSchema: z.object({
      id: z.number().describe("Note ID"),
    }),
    execute: async ({ id }) => {
      const res = await veta().fetch(`http://veta/notes/${id}`);
      if (!res.ok) return `Note ${id} not found.`;
      const note = (await res.json()) as { title: string; body: string; tags: string[] };
      return `# ${note.title}\n\n${note.body}\n\nTags: ${note.tags.join(", ")}`;
    },
  }),

  searchNotes: tool({
    description: "Search notes by pattern (regex supported)",
    inputSchema: z.object({
      query: z.string().describe("Search pattern"),
      tags: z.array(z.string()).optional().describe("Filter by these tags"),
    }),
    execute: async ({ query, tags }) => {
      const params = new URLSearchParams({ q: query });
      if (tags?.length) params.set("tags", tags.join(","));
      const res = await veta().fetch(`http://veta/grep?${params}`);
      if (!res.ok) {
        const error = await res.text();
        return `Search error: ${error}`;
      }
      const notes = (await res.json()) as { id: number; title: string; body?: string }[];
      if (!notes.length) return "No matching notes found.";
      return notes.map((n) => `[${n.id}] ${n.title}: ${(n.body || "").slice(0, 100)}...`).join("\n\n");
    },
  }),

  listTags: tool({
    description: "List all tags in the Veta knowledge base",
    inputSchema: z.object({}),
    execute: async () => {
      const res = await veta().fetch("http://veta/tags");
      const tags = (await res.json()) as { name: string; count: number }[];
      if (!tags.length) return "No tags found.";
      return tags.map((t) => `${t.name} (${t.count} notes)`).join("\n");
    },
  }),

  rmNotes: tool({
    description: "Delete one or more notes from the Veta knowledge base",
    inputSchema: z.object({
      ids: z.array(z.number()).describe("Note IDs to delete"),
    }),
    execute: async ({ ids }) => {
      const results: string[] = [];
      for (const id of ids) {
        const res = await veta().fetch(`http://veta/notes/${id}`, { method: "DELETE" });
        results.push(res.ok ? `Deleted note ${id}.` : `Note ${id} not found.`);
      }
      return results.join("\n");
    },
  }),
} satisfies ToolSet;
