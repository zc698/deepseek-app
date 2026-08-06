import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ChatEvent, ChatMessage, SessionMeta, Settings, SkillInfo } from "./lib/types";
import * as api from "./lib/api";
import { applyEvent } from "./lib/stream";
import Sidebar from "./components/Sidebar";
import ChatView from "./components/ChatView";
import SettingsModal from "./components/SettingsModal";
import SkillsModal from "./components/SkillsModal";

export default function App() {
  const [sessions, setSessions] = useState<SessionMeta[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [showSkills, setShowSkills] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Keep a ref of the active session so the global event listener can always see it.
  const activeRef = useRef<string | null>(null);
  activeRef.current = activeId;

  const refreshSessions = useCallback(async () => {
    const list = await api.sessionsList();
    setSessions(list);
  }, []);

  // Initial load
  useEffect(() => {
    (async () => {
      try {
        const [s, sk] = await Promise.all([api.settingsGet(), api.skillsList()]);
        setSettings(s);
        setSkills(sk);
        await refreshSessions();
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [refreshSessions]);

  // Subscribe to the host stream events once, forever.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen<ChatEvent>("chat://event", (evt) => {
        const payload = evt.payload;
        if (payload.sessionId && payload.sessionId !== activeRef.current) return;
        setMessages((prev) => applyEvent(prev, payload));
        if (payload.kind === "done" || payload.kind === "error") setBusy(false);
      });
    })();
    return () => unlisten?.();
  }, []);

  const selectSession = useCallback(async (id: string) => {
    setActiveId(id);
    setMessages(await api.sessionMessages(id));
  }, []);

  const newSession = useCallback(async () => {
    const id = await api.sessionCreate();
    setActiveId(id);
    setMessages([]);
    await refreshSessions();
  }, [refreshSessions]);

  const deleteSession = useCallback(
    async (id: string) => {
      await api.sessionDelete(id);
      if (id === activeRef.current) {
        setActiveId(null);
        setMessages([]);
      }
      await refreshSessions();
    },
    [refreshSessions],
  );

  const send = useCallback(
    async (text: string) => {
      if (!text.trim() || busy || !settings) return;
      setError(null);
      setBusy(true);
      const target = activeRef.current;
      try {
        await api.chatSend(target, text);
        if (!target) {
          // A new session was implicitly created by the host; refresh the list
          // and attach the event flow to it (messageId arrives via events).
          await refreshSessions();
          const list = await api.sessionsList();
          if (list.length > 0) setActiveId(list[0].id);
        } else {
          setMessages((prev) => [
            ...prev,
            {
              id: `local-${Date.now()}`,
              role: "user",
              content: text,
              reasoning: "",
              tools: [],
              createdAt: new Date().toISOString(),
            },
          ]);
        }
      } catch (e) {
        setError(String(e));
        setBusy(false);
      }
    },
    [busy, settings, refreshSessions],
  );

  const stop = useCallback(() => {
    if (activeRef.current) void api.chatStop(activeRef.current);
  }, []);

  const saveSettings = useCallback(
    async (next: Settings) => {
      const saved = await api.settingsSet(next);
      setSettings(saved);
      setShowSettings(false);
    },
    [],
  );

  if (loading) {
    return (
      <div className="flex h-screen items-center justify-center bg-white text-slate-500">
        加载中…
      </div>
    );
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-white text-slate-800">
      <Sidebar
        sessions={sessions}
        activeId={activeId}
        onSelect={selectSession}
        onNew={newSession}
        onDelete={deleteSession}
        onOpenSettings={() => setShowSettings(true)}
        onOpenSkills={() => setShowSkills(true)}
        model={settings?.model ?? ""}
      />
      <main className="flex flex-1 flex-col min-w-0">
        <ChatView
          messages={messages}
          busy={busy}
          onSend={send}
          onStop={stop}
          error={error}
          model={settings?.model ?? ""}
        />
      </main>

      {showSettings && settings && (
        <SettingsModal
          settings={settings}
          skills={skills}
          onSave={saveSettings}
          onClose={() => setShowSettings(false)}
        />
      )}
      {showSkills && (
        <SkillsModal skills={skills} onClose={() => setShowSkills(false)} />
      )}
    </div>
  );
}
