import type { ChatEvent, ChatMessage, ToolRecord } from "./types";

/**
 * Pure reducer for applying host stream events to the in-memory message list.
 * Kept free of Tauri/React imports so it can be unit tested in isolation.
 */
export function applyEvent(messages: ChatMessage[], ev: ChatEvent): ChatMessage[] {
  switch (ev.kind) {
    case "start": {
      const placeholder: ChatMessage = {
        id: ev.messageId,
        role: "assistant",
        content: "",
        reasoning: "",
        tools: [],
        streaming: true,
        createdAt: new Date().toISOString(),
      };
      return [...messages, placeholder];
    }

    case "stream": {
      return messages.map((m) => {
        if (m.id !== ev.messageId) return m;
        if (ev.reasoning) {
          return { ...m, reasoning: m.reasoning + ev.delta };
        }
        return { ...m, content: m.content + ev.delta };
      });
    }

    case "tool": {
      return messages.map((m) => {
        if (m.id !== ev.messageId) return m;
        const tools = upsertTool(m.tools, ev.tool);
        return { ...m, tools };
      });
    }

    case "done": {
      return messages.map((m) =>
        m.id === ev.messageId
          ? { ...m, content: ev.content, streaming: false, isError: false }
          : m,
      );
    }

    case "error": {
      return messages.map((m) =>
        m.id === ev.messageId
          ? { ...m, isError: true, streaming: false, content: m.content || ev.error }
          : m,
      );
    }

    default:
      return messages;
  }
}

function upsertTool(tools: ToolRecord[], incoming: ToolRecord): ToolRecord[] {
  const idx = tools.findIndex((t) => t.id === incoming.id);
  if (idx === -1) return [...tools, incoming];
  const next = [...tools];
  next[idx] = incoming;
  return next;
}
