import { describe, expect, it } from "vitest";
import { applyEvent } from "../stream";
import type { ChatMessage, ChatEvent } from "../types";

const base: ChatMessage[] = [];

describe("applyEvent", () => {
  it("appends an assistant placeholder on start", () => {
    const ev: ChatEvent = { kind: "start", sessionId: "s1", messageId: "m1" };
    const next = applyEvent(base, ev);
    expect(next).toHaveLength(1);
    expect(next[0].id).toBe("m1");
    expect(next[0].role).toBe("assistant");
    expect(next[0].streaming).toBe(true);
  });

  it("accumulates content deltas", () => {
    let msgs = applyEvent(base, { kind: "start", sessionId: "s1", messageId: "m1" });
    msgs = applyEvent(msgs, { kind: "stream", sessionId: "s1", messageId: "m1", delta: "你好", reasoning: false });
    msgs = applyEvent(msgs, { kind: "stream", sessionId: "s1", messageId: "m1", delta: "，世界", reasoning: false });
    expect(msgs[0].content).toBe("你好，世界");
  });

  it("accumulates reasoning deltas separately", () => {
    let msgs = applyEvent(base, { kind: "start", sessionId: "s1", messageId: "m1" });
    msgs = applyEvent(msgs, { kind: "stream", sessionId: "s1", messageId: "m1", delta: "思考中", reasoning: true });
    expect(msgs[0].reasoning).toBe("思考中");
    expect(msgs[0].content).toBe("");
  });

  it("upserts tool records by id and tracks status", () => {
    let msgs = applyEvent(base, { kind: "start", sessionId: "s1", messageId: "m1" });
    msgs = applyEvent(msgs, {
      kind: "tool",
      sessionId: "s1",
      messageId: "m1",
      tool: { id: "t1", name: "read_file", args: "{\"path\":\"a.txt\"}", status: "running" },
    });
    msgs = applyEvent(msgs, {
      kind: "tool",
      sessionId: "s1",
      messageId: "m1",
      tool: { id: "t1", name: "read_file", args: "{\"path\":\"a.txt\"}", status: "done", output: "hello" },
    });
    expect(msgs[0].tools).toHaveLength(1);
    expect(msgs[0].tools[0].status).toBe("done");
    expect(msgs[0].tools[0].output).toBe("hello");
  });

  it("finalizes on done and clears streaming flag", () => {
    let msgs = applyEvent(base, { kind: "start", sessionId: "s1", messageId: "m1" });
    msgs = applyEvent(msgs, { kind: "done", sessionId: "s1", messageId: "m1", content: "最终答案" });
    expect(msgs[0].streaming).toBe(false);
    expect(msgs[0].content).toBe("最终答案");
  });

  it("marks errors", () => {
    let msgs = applyEvent(base, { kind: "start", sessionId: "s1", messageId: "m1" });
    msgs = applyEvent(msgs, { kind: "error", sessionId: "s1", messageId: "m1", error: "API 认证失败" });
    expect(msgs[0].isError).toBe(true);
    expect(msgs[0].streaming).toBe(false);
  });

  it("applies start events regardless of sessionId (filter happens upstream)", () => {
    // The reducer is session-agnostic on purpose: filtering is done by App.tsx
    // before calling applyEvent so the reducer stays a pure, testable function.
    let msgs = applyEvent(base, { kind: "start", sessionId: "other", messageId: "mX" });
    expect(msgs).toHaveLength(1);
  });
});
