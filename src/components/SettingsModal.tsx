import { useState } from "react";
import { Loader2, Plug, Save, X } from "lucide-react";
import type { Settings, SkillInfo } from "../lib/types";
import { modelsList, pingProvider } from "../lib/api";

interface Props {
  settings: Settings;
  skills: SkillInfo[];
  onSave: (s: Settings) => void;
  onClose: () => void;
}

const MODEL_CATALOG = [
  { id: "deepseek-v4-flash", name: "DeepSeek V4 Flash（快速高性价比，1M 上下文）" },
  { id: "deepseek-v4-pro", name: "DeepSeek V4 Pro（更强推理，1M 上下文）" },
];

// Legacy model ids (deprecated V3 era) -> current V4 successors. Kept so users
// who saved deepseek-chat / deepseek-reasoner before the upgrade still see the
// right model without losing any of their other settings.
const MODEL_MIGRATION: Record<string, string> = {
  "deepseek-chat": "deepseek-v4-flash",
  "deepseek-reasoner": "deepseek-v4-pro",
};

const migrateModel = (id: string): string => MODEL_MIGRATION[id] ?? id;

export default function SettingsModal({ settings, skills, onSave, onClose }: Props) {
  const [form, setForm] = useState<Settings>({ ...settings, model: migrateModel(settings.model) });
  const [customModel, setCustomModel] = useState(
    MODEL_CATALOG.some((m) => m.id === migrateModel(settings.model))
      ? ""
      : migrateModel(settings.model),
  );
  const [pinging, setPinging] = useState(false);
  const [pingResult, setPingResult] = useState<{ ok: boolean; text: string } | null>(null);

  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setForm((f) => ({ ...f, [key]: value }));

  const doPing = async () => {
    setPinging(true);
    setPingResult(null);
    try {
      const res = await pingProvider(form.api_key, form.base_url);
      if (res.ok) {
        setPingResult({ ok: true, text: `连接成功，可用模型：${res.models.join(", ")}` });
      } else {
        setPingResult({ ok: false, text: res.error || "连接失败" });
      }
    } catch (e) {
      setPingResult({ ok: false, text: String(e) });
    } finally {
      setPinging(false);
    }
  };

  const doSave = async () => {
    const model = migrateModel(customModel.trim() || form.model);
    await onSave({ ...form, model });
  };

  const input =
    "w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 outline-none focus:border-[#4D6BFE]";
  const label = "mb-1 block text-xs font-medium text-slate-500";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30" onClick={onClose}>
      <div
        className="max-h-[85vh] w-[560px] overflow-y-auto rounded-2xl bg-white p-6 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-base font-semibold text-slate-900">设置</h2>
          <button onClick={onClose} className="rounded p-1 text-slate-400 hover:text-slate-600">
            <X size={18} />
          </button>
        </div>

        <div className="space-y-4">
          <div>
            <label className={label}>DeepSeek API Key（platform.deepseek.com）</label>
            <input
              type="password"
              className={input}
              placeholder="sk-..."
              value={form.api_key}
              onChange={(e) => set("api_key", e.target.value)}
            />
            <p className="mt-1 text-[11px] text-slate-400">仅保存在本机，用于直连 DeepSeek 官方 API。</p>
          </div>

          <div>
            <label className={label}>API Base URL</label>
            <input
              className={input}
              value={form.base_url}
              onChange={(e) => set("base_url", e.target.value)}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className={label}>模型</label>
              <select
                className={input}
                value={customModel ? "__custom__" : form.model}
                onChange={(e) => {
                  if (e.target.value === "__custom__") setCustomModel(form.model);
                  else {
                    setCustomModel("");
                    set("model", e.target.value);
                  }
                }}
              >
                {MODEL_CATALOG.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
                <option value="__custom__">自定义…</option>
              </select>
              {MODEL_MIGRATION[settings.model] && (
                <p className="mt-1 text-[11px] text-amber-600">
                  已停用的旧模型「{settings.model}」已自动迁移至「{MODEL_MIGRATION[settings.model]}」，其余设置保持不变。
                </p>
              )}
            </div>
            <div>
              <label className={label}>自定义模型 ID</label>
              <input
                className={input}
                placeholder="例如 deepseek-v4.1"
                value={customModel}
                onChange={(e) => setCustomModel(e.target.value)}
              />
            </div>
          </div>

          <div>
            <label className={label}>Temperature：{form.temperature.toFixed(1)}</label>
            <input
              type="range"
              min={0}
              max={2}
              step={0.1}
              className="w-full accent-[#4D6BFE]"
              value={form.temperature}
              onChange={(e) => set("temperature", parseFloat(e.target.value))}
            />
          </div>

          <div>
            <label className={label}>工作目录（工具读写文件的范围）</label>
            <input className={input} value={form.workspace_dir} onChange={(e) => set("workspace_dir", e.target.value)} />
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="allow-bash"
              className="h-4 w-4 accent-[#4D6BFE]"
              checked={form.allow_bash}
              onChange={(e) => set("allow_bash", e.target.checked)}
            />
            <label htmlFor="allow-bash" className="text-sm text-slate-600">
              允许执行 Bash 命令（默认关闭，仅在你信任时才开启）
            </label>
          </div>

          <div>
            <label className={label}>系统提示词（附加）</label>
            <textarea
              className={`${input} min-h-20`}
              value={form.system_prompt}
              onChange={(e) => set("system_prompt", e.target.value)}
            />
          </div>

          <div>
            <label className={label}>技能（{skills.filter((s) => s.enabled).length}/{skills.length} 已启用）</label>
            <div className="space-y-1">
              {skills.map((s) => (
                <div key={s.name} className="flex items-center gap-2 text-sm text-slate-600">
                  <input
                    type="checkbox"
                    className="h-4 w-4 accent-[#4D6BFE]"
                    checked={form.enabled_skills.includes(s.name)}
                    onChange={(e) => {
                      const cur = form.enabled_skills;
                      set(
                        "enabled_skills",
                        e.target.checked
                          ? [...cur, s.name]
                          : cur.filter((n) => n !== s.name),
                      );
                    }}
                  />
                  <span className="font-mono text-xs">{s.name}</span>
                </div>
              ))}
              {skills.length === 0 && (
                <p className="text-xs text-slate-400">未发现技能</p>
              )}
            </div>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => void doPing()}
              disabled={pinging || !form.api_key}
              className="flex items-center gap-1.5 rounded-lg border border-slate-200 px-3 py-2 text-sm text-slate-600 transition hover:bg-slate-50 disabled:opacity-40"
            >
              {pinging ? <Loader2 size={14} className="animate-spin" /> : <Plug size={14} />}
              测试连接
            </button>
            {pingResult && (
              <span className={`text-xs ${pingResult.ok ? "text-emerald-600" : "text-red-500"}`}>
                {pingResult.text}
              </span>
            )}
          </div>
        </div>

        <div className="mt-6 flex justify-end gap-2 border-t border-slate-100 pt-4">
          <button
            onClick={onClose}
            className="rounded-lg px-4 py-2 text-sm text-slate-500 transition hover:bg-slate-100"
          >
            取消
          </button>
          <button
            onClick={() => void doSave()}
            className="flex items-center gap-1.5 rounded-lg bg-[#4D6BFE] px-4 py-2 text-sm font-medium text-white transition hover:bg-[#3d5ae8]"
          >
            <Save size={14} /> 保存
          </button>
        </div>
      </div>
    </div>
  );
}
