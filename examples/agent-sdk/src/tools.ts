import { tool, type ToolSet } from "ai";
import { z } from "zod";

export const tools = {
  addNote: tool({
    description: "Add a note to the Veta knowledge base",
    parameters: z.object({
      title: z.string().describe("Note title"),
      body: z.string().describe("Note content"),
      tags: z.array(z.string()).describe("Tags for organization"),
    }),
    execute: async ({ title, body, tags }) => {
      const res = await fetch("http://veta/notes", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ title, body, tags }),
      });
      const data = await res.json();
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
      const res = await fetch(`http://veta/notes${params}`);
      const notes = await res.json();
      if (!notes.length) return "No notes found.";
      return notes.map((n: any) => `[${n.id}] ${n.title} (${n.tags.join(", ")})`).join("\n");
    },
  }),

  showNote: tool({
    description: "Show the full content of a specific note",
    parameters: z.object({
      id: z.number().describe("Note ID"),
    }),
    execute: async ({ id }) => {
      const res = await fetch(`http://veta/notes/${id}`);
      if (!res.ok) return `Note ${id} not found.`;
      const note = await res.json();
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
      const res = await fetch(`http://veta/grep?${params}`);
      const notes = await res.json();
      if (!notes.length) return "No matching notes found.";
      return notes.map((n: any) => `[${n.id}] ${n.title}: ${n.body.slice(0, 100)}...`).join("\n\n");
    },
  }),

  listTags: tool({
    description: "List all tags in the Veta knowledge base",
    parameters: z.object({}),
    execute: async () => {
      const res = await fetch("http://veta/tags");
      const tags = await res.json();
      if (!tags.length) return "No tags found.";
      return tags.map((t: any) => `${t.name} (${t.count} notes)`).join("\n");
    },
  }),

  deleteNote: tool({
    description: "Delete a note from the Veta knowledge base",
    parameters: z.object({
      id: z.number().describe("Note ID to delete"),
    }),
    execute: async ({ id }) => {
      const res = await fetch(`http://veta/notes/${id}`, { method: "DELETE" });
      return res.ok ? `Deleted note ${id}.` : `Failed to delete note ${id}.`;
    },
  }),
} satisfies ToolSet;

export const createExecutions = (_env: Env) => ({});
