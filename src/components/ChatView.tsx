import { useEffect, useRef, useState } from "react";
import { AlertCircle, Brain, Send, Square, Wrench } from "lucide-react";
import type { ChatMessage } from "../lib/types";
import Markdown from "./Markdown";

interface Props {
  messages: ChatMessage[];
  busy: boolean;
  error: string | null;
  model: string;
  onSend: (text: string) => void;
  onStop: () => void;
}

function ToolChip({ tool }: { tool: ChatMessage["tools"][number] }) {
  const icon =
    tool.status === "running" ? (
      <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-[#4D6BFE] border-t-transparent" />
    ) : tool.status === "done" ? (
      <span className="text-emerald-600">✓</span>
    ) : (
      <span className="text-red-500">✕</span>
    );
  return (
    <div className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs">
      <div className="flex items-center gap-1.5 font-medium text-slate-700">
        {icon}
        <Wrench size={12} className="opacity-60" />
        <span className="font-mono">{tool.name}</span>
      </div>
      <div className="mt-1 truncate font-mono text-[11px] text-slate-400">{tool.args}</div>
      {tool.output && (
        <details className="mt-1">
          <summary className="cursor-pointer text-[11px] text-slate-400">输出</summary>
          <pre className="mt-1 max-h-40 overflow-auto rounded bg-white p-2 text-[11px] text-slate-600">
            {tool.output.slice(0, 4000)}
          </pre>
        </details>
      )}
    </div>
  );
}

function AssistantBody({ message }: { message: ChatMessage }) {
  return (
    <div className="space-y-2">
      {message.reasoning && (
        <details className="group rounded-lg border border-amber-200 bg-amber-50/70 px-3 py-2">
          <summary className="flex cursor-pointer list-none items-center gap-1.5 text-xs font-medium text-amber-700">
            <Brain size={13} />
            思考过程
            <span className="text-amber-400 group-open:hidden">展开</span>
            <span className="hidden text-amber-400 group-open:inline">收起</span>
          </summary>
          <div className="mt-2 whitespace-pre-wrap text-xs leading-relaxed text-amber-800/90">
            {message.reasoning}
          </div>
        </details>
      )}
      {message.tools.length > 0 && (
        <div className="space-y-1.5">
          {message.tools.map((t) => (
            <ToolChip key={t.id} tool={t} />
          ))}
        </div>
      )}
      <Markdown content={message.content} />
      {message.streaming && !message.content && !message.reasoning && (
        <div className="flex gap-1 py-1 text-[#4D6BFE]">
          <span className="h-2 w-2 animate-bounce rounded-full bg-[#4D6BFE]" />
          <span className="h-2 w-2 animate-bounce rounded-full bg-[#4D6BFE] [animation-delay:0.15s]" />
          <span className="h-2 w-2 animate-bounce rounded-full bg-[#4D6BFE] [animation-delay:0.3s]" />
        </div>
      )}
    </div>
  );
}

export default function ChatView({ messages, busy, error, model, onSend, onStop }: Props) {
  const [draft, setDraft] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const submit = () => {
    const text = draft.trim();
    if (!text || busy) return;
    onSend(text);
    setDraft("");
    if (taRef.current) taRef.current.style.height = "auto";
  };

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center justify-between border-b border-slate-200 px-5 py-3">
        <h1 className="text-sm font-semibold text-slate-700">对话</h1>
        <span className="rounded-full bg-slate-100 px-2.5 py-0.5 text-xs text-slate-500">
          {model}
        </span>
      </header>

      <div className="flex-1 overflow-y-auto px-6 py-4">
        {messages.length === 0 && (
          <div className="mt-16 text-center">
            <div className="mx-auto mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-[#4D6BFE]/10 text-[#4D6BFE]">
              <Brain size={28} />
            </div>
            <h2 className="text-lg font-semibold text-slate-800">你好，我是 DeepSeek</h2>
            <p className="mx-auto mt-1 max-w-sm text-sm text-slate-400">
              由 DeepSeek 官方 API 驱动，支持技能、工具调用与流式思考。
            </p>
          </div>
        )}

        {messages.map((m) => (
          <div
            key={m.id}
            className={`mb-4 flex ${m.role === "user" ? "justify-end" : "justify-start"}`}
          >
            {m.role === "user" ? (
              <div className="max-w-[75%] whitespace-pre-wrap rounded-2xl rounded-br-md bg-[#4D6BFE] px-4 py-2.5 text-sm leading-relaxed text-white">
                {m.content}
              </div>
            ) : (
              <div
                className={`max-w-[85%] min-w-0 rounded-2xl rounded-bl-md border px-4 py-3 ${
                  m.isError ? "border-red-200 bg-red-50" : "border-slate-200 bg-white"
                }`}
              >
                <AssistantBody message={m} />
              </div>
            )}
          </div>
        ))}

        {error && (
          <div className="mb-4 flex items-center gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-600">
            <AlertCircle size={14} /> {error}
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      <div className="border-t border-slate-200 bg-slate-50/70 px-5 py-3">
        <div className="mx-auto flex max-w-3xl items-end gap-2 rounded-2xl border border-slate-200 bg-white p-2 shadow-sm focus-within:border-[#4D6BFE]">
          <textarea
            ref={taRef}
            value={draft}
            rows={1}
            placeholder="输入消息…（Enter 发送，Shift+Enter 换行）"
            className="max-h-40 flex-1 resize-none bg-transparent px-2 py-1.5 text-sm outline-none"
            onChange={(e) => {
              setDraft(e.target.value);
              e.target.style.height = "auto";
              e.target.style.height = Math.min(e.target.scrollHeight, 160) + "px";
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
          />
          {busy ? (
            <button
              onClick={onStop}
              className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-slate-800 text-white transition hover:bg-slate-600"
              title="停止生成"
            >
              <Square size={13} />
            </button>
          ) : (
            <button
              onClick={submit}
              disabled={!draft.trim()}
              className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-[#4D6BFE] text-white transition enabled:hover:bg-[#3d5ae8] disabled:opacity-40"
              title="发送"
            >
              <Send size={14} />
            </button>
          )}
        </div>
        <p className="mt-1.5 text-center text-[11px] text-slate-400">
          DeepSeek 生成的内容仅供参考，请自行核实重要信息
        </p>
      </div>
    </div>
  );
}
