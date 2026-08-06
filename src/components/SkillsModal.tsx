import { X } from "lucide-react";
import type { SkillInfo } from "../lib/types";

interface Props {
  skills: SkillInfo[];
  onClose: () => void;
}

export default function SkillsModal({ skills, onClose }: Props) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30" onClick={onClose}>
      <div
        className="max-h-[85vh] w-[560px] overflow-y-auto rounded-2xl bg-white p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-1 flex items-center justify-between">
          <h2 className="text-base font-semibold text-slate-900">技能（Skills）</h2>
          <button onClick={onClose} className="rounded p-1 text-slate-400 hover:text-slate-600">
            <X size={18} />
          </button>
        </div>
        <p className="mb-4 text-xs text-slate-400">
          技能采用 <code className="rounded bg-slate-100 px-1">SKILL.md</code>{" "}
          （YAML frontmatter + Markdown 正文）定义。发送消息时由模型自动匹配并注入相关技能。
        </p>

        <div className="space-y-2">
          {skills.map((s) => (
            <div key={s.path} className="rounded-xl border border-slate-200 p-3">
              <div className="flex items-center gap-2">
                <span
                  className={`h-1.5 w-1.5 rounded-full ${
                    s.enabled ? "bg-emerald-500" : "bg-slate-300"
                  }`}
                />
                <span className="font-mono text-sm font-medium text-slate-800">{s.name}</span>
                {!s.implicit && (
                  <span className="rounded bg-amber-100 px-1.5 py-0.5 text-[10px] text-amber-700">
                    手动触发
                  </span>
                )}
              </div>
              <p className="mt-1 text-xs text-slate-500">{s.description}</p>
              <p className="mt-1 truncate font-mono text-[10px] text-slate-300">{s.path}</p>
            </div>
          ))}
          {skills.length === 0 && (
            <p className="text-sm text-slate-400">未发现技能。</p>
          )}
        </div>
      </div>
    </div>
  );
}
