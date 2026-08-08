// ---- Shared data types (mirror the Rust-side models) ----

export interface ToolRecord {
  id: string;
  name: string;
  args: string;
  status: "running" | "done" | "error";
  output?: string;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  reasoning: string;
  tools: ToolRecord[];
  streaming?: boolean;
  isError?: boolean;
  createdAt: string;
}

export interface SessionMeta {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface Settings {
  api_key: string;
  base_url: string;
  model: string;
  temperature: number;
  system_prompt: string;
  allow_bash: boolean;
  workspace_dir: string;
  max_tool_rounds: number;
  enabled_skills: string[];
}

export interface SkillInfo {
  name: string;
  path: string;
  description: string;
  implicit: boolean;
  enabled: boolean;
}

export interface Workspace {
  id: string;
  name: string;
  path: string;
}

export interface WorkspaceState {
  current: string | null;
  items: Workspace[];
}

export type ChatEvent =
  | { kind: "start"; sessionId: string; messageId: string }
  | { kind: "stream"; sessionId: string; messageId: string; delta: string; reasoning: boolean }
  | { kind: "tool"; sessionId: string; messageId: string; tool: ToolRecord }
  | { kind: "done"; sessionId: string; messageId: string; content: string }
  | { kind: "error"; sessionId: string; messageId: string; error: string };
