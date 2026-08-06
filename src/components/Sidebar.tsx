import { MessageSquare, Plus, Settings, Sparkles, Trash2, X } from "lucide-react";
import type { SessionMeta } from "../lib/types";

interface Props {
  sessions: SessionMeta[];
  activeId: string | null;
  model: string;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
  onOpenSkills: () => void;
}

export default function Sidebar({
  sessions,
  activeId,
  model,
  onSelect,
  onNew,
  onDelete,
  onOpenSettings,
  onOpenSkills,
}: Props) {
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

      <div className="flex-1 overflow-y-auto px-2 pb-2">
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
