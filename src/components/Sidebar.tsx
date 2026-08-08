import { useState } from "react";
import { Check, Folder, MessageSquare, Plus, Settings, Sparkles, X } from "lucide-react";
import type { SessionMeta, WorkspaceState } from "../lib/types";

interface Props {
  sessions: SessionMeta[];
  activeId: string | null;
  model: string;
  workspaces: WorkspaceState;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
  onOpenSkills: () => void;
  onWorkspaceSelect: (id: string) => void;
  onWorkspaceAdd: (name: string, path: string) => Promise<void>;
  onWorkspaceRemove: (id: string) => void;
}

export default function Sidebar({
  sessions,
  activeId,
  model,
  workspaces,
  onSelect,
  onNew,
  onDelete,
  onOpenSettings,
  onOpenSkills,
  onWorkspaceSelect,
  onWorkspaceAdd,
  onWorkspaceRemove,
}: Props) {
  const [adding, setAdding] = useState(false);
  const [wsName, setWsName] = useState("");
  const [wsPath, setWsPath] = useState("");

  const submitAdd = async () => {
    if (!wsPath.trim()) return;
    try {
      await onWorkspaceAdd(wsName, wsPath);
      setAdding(false);
      setWsName("");
      setWsPath("");
    } catch {
      // error surfaces through the app-level error banner
    }
  };

  const currentWorkspace = workspaces.items.find((w) => w.id === workspaces.current) ?? null;

  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-slate-200 bg-slate-50">
      <div className="flex items-center gap-2 px-4 py-4">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-[#4D6BFE] text-white">
          <Sparkles size={18} />
        </div>
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold text-slate-900">DeepSeek App</div>
          <div className="truncate text-xs text-slate-400">{model}</div>
        </div>
      </div>

      <div className="px-3 pb-2">
        <button
          onClick={onNew}
          className="flex w-full items-center justify-center gap-1.5 rounded-lg bg-[#4D6BFE] px-3 py-2 text-sm font-medium text-white transition hover:bg-[#3d5ae8]"
        >
          <Plus size={16} /> 新对话
        </button>
      </div>

      {/* Workspaces */}
      <div className="border-t border-slate-200 px-2 pt-2">
        <div className="mb-1 flex items-center justify-between px-1">
          <span className="text-[11px] font-medium uppercase tracking-wide text-slate-400">
            工作区
          </span>
          <button
            onClick={() => setAdding((v) => !v)}
            className="rounded p-0.5 text-slate-400 transition hover:bg-slate-200 hover:text-slate-600"
            title="添加工作区"
          >
            <Plus size={14} />
          </button>
        </div>

        {adding && (
          <div className="mb-1 space-y-1 rounded-lg border border-slate-200 bg-white p-2">
            <input
              autoFocus
              value={wsName}
              onChange={(e) => setWsName(e.target.value)}
              placeholder="名称（可选）"
              className="w-full rounded border border-slate-200 px-2 py-1 text-xs outline-none focus:border-[#4D6BFE]"
              onKeyDown={(e) => e.key === "Enter" && void submitAdd()}
            />
            <input
              value={wsPath}
              onChange={(e) => setWsPath(e.target.value)}
              placeholder="目录路径，如 /Users/you/project"
              className="w-full rounded border border-slate-200 px-2 py-1 text-xs outline-none focus:border-[#4D6BFE]"
              onKeyDown={(e) => e.key === "Enter" && void submitAdd()}
            />
            <div className="flex justify-end gap-1">
              <button
                onClick={() => setAdding(false)}
                className="rounded px-2 py-0.5 text-[11px] text-slate-500 hover:bg-slate-100"
              >
                取消
              </button>
              <button
                onClick={() => void submitAdd()}
                disabled={!wsPath.trim()}
                className="rounded bg-[#4D6BFE] px-2 py-0.5 text-[11px] text-white hover:bg-[#3d5ae8] disabled:opacity-40"
              >
                添加
              </button>
            </div>
          </div>
        )}

        <div className="max-h-40 overflow-y-auto">
          {workspaces.items.length === 0 && (
            <div className="px-3 py-2 text-[11px] text-slate-400">暂无工作区，点击 + 添加</div>
          )}
          {workspaces.items.map((w) => (
            <div
              key={w.id}
              className={`group mb-0.5 flex cursor-pointer items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs transition ${
                w.id === workspaces.current
                  ? "bg-[#4D6BFE]/10 text-[#3d5ae8]"
                  : "text-slate-600 hover:bg-slate-200/70"
              }`}
              onClick={() => onWorkspaceSelect(w.id)}
              title={w.path}
            >
              <Folder size={13} className="shrink-0 opacity-60" />
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium">{w.name}</span>
                <span className="block truncate text-[10px] opacity-60">{w.path}</span>
              </span>
              {w.id === workspaces.current && <Check size={12} className="shrink-0" />}
              <button
                className="hidden shrink-0 rounded p-0.5 text-slate-400 hover:text-red-500 group-hover:block"
                title="移除工作区"
                onClick={(e) => {
                  e.stopPropagation();
                  onWorkspaceRemove(w.id);
                }}
              >
                <X size={12} />
              </button>
            </div>
          ))}
        </div>
        {currentWorkspace && (
          <div className="mt-1 truncate rounded bg-slate-100 px-2 py-1 text-[10px] text-slate-400">
            Agent 将在「{currentWorkspace.name}」目录内工作
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto px-2 pb-2 pt-1">
        {sessions.length === 0 && (
          <div className="px-3 py-6 text-center text-xs text-slate-400">
            还没有会话，点击上方"新对话"开始
          </div>
        )}
        {sessions.map((s) => (
          <div
            key={s.id}
            className={`group mb-0.5 flex cursor-pointer items-center gap-2 rounded-lg px-3 py-2 text-sm transition ${
              s.id === activeId
                ? "bg-[#4D6BFE]/10 text-[#3d5ae8]"
                : "text-slate-600 hover:bg-slate-200/70"
            }`}
            onClick={() => onSelect(s.id)}
          >
            <MessageSquare size={14} className="shrink-0 opacity-60" />
            <span className="flex-1 truncate">{s.title || "新对话"}</span>
            <button
              className="hidden shrink-0 rounded p-0.5 text-slate-400 hover:text-red-500 group-hover:block"
              title="删除会话"
              onClick={(e) => {
                e.stopPropagation();
                onDelete(s.id);
              }}
            >
              <X size={13} />
            </button>
          </div>
        ))}
      </div>

      <div className="border-t border-slate-200 p-2">
        <button
          onClick={onOpenSkills}
          className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-600 transition hover:bg-slate-200/70"
        >
          <Sparkles size={15} className="opacity-70" /> 技能
        </button>
        <button
          onClick={onOpenSettings}
          className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-slate-600 transition hover:bg-slate-200/70"
        >
          <Settings size={15} className="opacity-70" /> 设置
        </button>
      </div>
    </aside>
  );
}
