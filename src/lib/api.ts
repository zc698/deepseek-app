import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage, SessionMeta, Settings, SkillInfo, WorkspaceState } from "./types";

// ---- IPC wrappers (typed invoke calls into the Rust host) ----

export function chatSend(
  sessionId: string | null,
  text: string,
): Promise<{ sessionId: string; messageId: string }> {
  return invoke("chat_send", { sessionId, text });
}

export function chatStop(sessionId: string): Promise<void> {
  return invoke("chat_stop", { sessionId });
}

export function sessionsList(): Promise<SessionMeta[]> {
  return invoke("sessions_list");
}

export function sessionCreate(): Promise<string> {
  return invoke("session_create");
}

export function sessionDelete(id: string): Promise<void> {
  return invoke("session_delete", { id });
}

export function sessionMessages(id: string): Promise<ChatMessage[]> {
  return invoke("session_messages", { id });
}

export function settingsGet(): Promise<Settings> {
  return invoke("settings_get");
}

export function settingsSet(settings: Settings): Promise<Settings> {
  return invoke("settings_set", { settings });
}

export function skillsList(): Promise<SkillInfo[]> {
  return invoke("skills_list");
}

export function modelsList(): Promise<{ id: string; name: string }[]> {
  return invoke("models_list");
}

export function pingProvider(
  apiKey: string,
  baseUrl: string,
): Promise<{ ok: boolean; models: string[]; error?: string }> {
  return invoke("ping_provider", { apiKey, baseUrl });
}

export function workspacesList(): Promise<WorkspaceState> {
  return invoke("workspaces_list");
}

export function workspacesAdd(name: string, path: string): Promise<WorkspaceState> {
  return invoke("workspaces_add", { name, path });
}

export function workspacesRemove(id: string): Promise<WorkspaceState> {
  return invoke("workspaces_remove", { id });
}

export function workspacesSetCurrent(id: string): Promise<WorkspaceState> {
  return invoke("workspaces_set_current", { id });
}
