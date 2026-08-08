import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { ChatEvent, ChatMessage, SessionMeta, Settings, SkillInfo } from "./lib/types";
import * as api from "./lib/api";
import { applyEvent, shouldAcceptEvent } from "./lib/stream";
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
  const [busy, setBusyState] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Keep refs of the active session and busy flag so the global event listener
  // (subscribed once, with empty deps) always sees the latest values.
  const activeRef = useRef<string | null>(null);
  activeRef.current = activeId;
  const busyRef = useRef(false);

  const setBusy = useCallback((v: boolean) => {
    busyRef.current = v;
    setBusyState(v);
  }, []);

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
        if (!shouldAcceptEvent(activeRef.current, busyRef.current, payload)) return;
        // New-session race: the host may emit `start` for a brand-new session
        // before chat_send's invoke response resolves; adopt the session id so
        // the rest of the stream is not filtered out.
        if (payload.kind === "start" && payload.sessionId && payload.sessionId !== activeRef.current) {
          activeRef.current = payload.sessionId;
          setActiveId(payload.sessionId);
        }
        setMessages((prev) => applyEvent(prev, payload));
        if (payload.kind === "done" || payload.kind === "error") setBusy(false);
      });
    })();
    return () => unlisten?.();
  }, [setBusy]);

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
      if (!text.trim() || busyRef.current || !settings) return;
      setError(null);
      setBusy(true);
      const target = activeRef.current;
      // Optimistically show the user message BEFORE any stream events, so the
      // message order is always user-then-assistant even for a brand-new session.
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
      try {
        const res = await api.chatSend(target, text);
        if (!target) {
          // The host created a new session; attach the event flow to it
          // immediately (closes the race with the incoming `start` event).
          activeRef.current = res.sessionId;
          setActiveId(res.sessionId);
          await refreshSessions();
        }
      } catch (e) {
        setError(String(e));
        setBusy(false);
      }
    },
    [settings, refreshSessions, setBusy],
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

  // Switch the model directly from the chat view (persists like settings).
  const changeModel = useCallback(
    async (model: string) => {
      if (!settings) return;
      setError(null);
      try {
        const saved = await api.settingsSet({ ...settings, model });
        setSettings(saved);
      } catch (e) {
        setError(String(e));
      }
    },
    [settings],
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
          onModelChange={changeModel}
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
