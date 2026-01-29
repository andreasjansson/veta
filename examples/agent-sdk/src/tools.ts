import { tool, type ToolSet } from "ai";
import { z } from "zod";
import { getCurrentAgent } from "agents";
import type { Chat } from "./agent";

function veta() {
  const { agent } = getCurrentAgent<Chat>();
  // @ts-expect-error - env is protected but accessible at runtime
  return agent!.env.VETA as Fetcher;
}

export const tools = {
  addNote: tool({
    description: "Add a note to the Veta knowledge base",
    parameters: z.object({
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
      const data = (await res.json()) as { id: number };
      return `Added note ${data.id}: ${title}`;
    },
  }),

  listNotes: tool({
    description: "List notes from the Veta knowledge base, optionally filtered by tags",
    parameters: z.object({
      tags: z.array(z.string()).optional().describe("Filter by these tags"),
    }),
    execute: async ({ tags }) => {
      const params = tags?.length ? `?tags=${tags.join(",")}` : "";
      const res = await veta().fetch(`http://veta/notes${params}`);
      const notes = (await res.json()) as { id: number; title: string; tags: string[] }[];
      if (!notes.length) return "No notes found.";
      return notes.map((n) => `[${n.id}] ${n.title} (${n.tags.join(", ")})`).join("\n");
    },
  }),

  showNote: tool({
    description: "Show the full content of a specific note",
    parameters: z.object({
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
    parameters: z.object({
      query: z.string().describe("Search pattern"),
      tags: z.array(z.string()).optional().describe("Filter by these tags"),
    }),
    execute: async ({ query, tags }) => {
      const params = new URLSearchParams({ q: query });
      if (tags?.length) params.set("tags", tags.join(","));
      const res = await veta().fetch(`http://veta/grep?${params}`);
      const notes = (await res.json()) as { id: number; title: string; body: string }[];
      if (!notes.length) return "No matching notes found.";
      return notes.map((n) => `[${n.id}] ${n.title}: ${n.body.slice(0, 100)}...`).join("\n\n");
    },
  }),

  listTags: tool({
    description: "List all tags in the Veta knowledge base",
    parameters: z.object({}),
    execute: async () => {
      const res = await veta().fetch("http://veta/tags");
      const tags = (await res.json()) as { name: string; count: number }[];
      if (!tags.length) return "No tags found.";
      return tags.map((t) => `${t.name} (${t.count} notes)`).join("\n");
    },
  }),

  deleteNote: tool({
    description: "Delete a note from the Veta knowledge base",
    parameters: z.object({
      id: z.number().describe("Note ID to delete"),
    }),
    execute: async ({ id }) => {
      const res = await veta().fetch(`http://veta/notes/${id}`, { method: "DELETE" });
      return res.ok ? `Deleted note ${id}.` : `Failed to delete note ${id}.`;
    },
  }),
} satisfies ToolSet;
