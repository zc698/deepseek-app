import { describe, expect, it } from "vitest";
import { MODEL_CATALOG, MODEL_MIGRATION, migrateModel } from "../models";

describe("models", () => {
  it("catalog contains only current V4 models", () => {
    expect(MODEL_CATALOG.map((m) => m.id)).toEqual(["deepseek-v4-flash", "deepseek-v4-pro"]);
  });

  it("every legacy id maps to an id present in the catalog", () => {
    for (const [legacy, current] of Object.entries(MODEL_MIGRATION)) {
      expect(MODEL_CATALOG.some((m) => m.id === current)).toBe(true);
      expect(legacy).not.toBe(current);
    }
  });

  it("migrateModel maps legacy ids and passes everything else through", () => {
    expect(migrateModel("deepseek-chat")).toBe("deepseek-v4-flash");
    expect(migrateModel("deepseek-reasoner")).toBe("deepseek-v4-pro");
    expect(migrateModel("deepseek-v4-flash")).toBe("deepseek-v4-flash");
    expect(migrateModel("custom-x")).toBe("custom-x");
  });
});
