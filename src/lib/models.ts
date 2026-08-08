// Model catalog shared by the chat view switcher and the settings panel.
// Kept framework-free so it can be unit tested in isolation.

export interface ModelOption {
  id: string;
  name: string;
}

export const MODEL_CATALOG: ModelOption[] = [
  { id: "deepseek-v4-flash", name: "DeepSeek V4 Flash（快速高性价比）" },
  { id: "deepseek-v4-pro", name: "DeepSeek V4 Pro（更强推理）" },
];

// Legacy (deprecated V3 era) ids -> current V4 successors. Used so users who
// saved an old model id still see the right model without losing settings.
export const MODEL_MIGRATION: Record<string, string> = {
  "deepseek-chat": "deepseek-v4-flash",
  "deepseek-reasoner": "deepseek-v4-pro",
};

export const migrateModel = (id: string): string => MODEL_MIGRATION[id] ?? id;
