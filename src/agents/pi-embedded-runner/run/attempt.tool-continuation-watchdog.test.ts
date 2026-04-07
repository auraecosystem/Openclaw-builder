import { describe, expect, it } from "vitest";
import {
  createToolContinuationWatchdog,
  POST_TOOL_CONTINUATION_TIMEOUT_ERROR_TEXT,
  type ToolContinuationWatchdogAgentEvent,
} from "./tool-continuation-watchdog.js";

describe("tool continuation watchdog", () => {
  it("aborts after a terminal tool result with no continuation and emits one failure note", async () => {
    let now = 1_000;
    let scheduled: { fn: () => void; delay: number } | undefined;
    const notes: string[] = [];
    const timeouts: Error[] = [];

    const watchdog = createToolContinuationWatchdog({
      runId: "run-1",
      sessionId: "session-1",
      sessionKey: "agent:main:discord:channel:test",
      timeoutMs: 20_000,
      now: () => now,
      setTimer: ((fn: () => void, delay?: number) => {
        scheduled = { fn, delay: delay ?? 0 };
        return scheduled as unknown as ReturnType<typeof setTimeout>;
      }) as typeof setTimeout,
      clearTimer: (() => {
        scheduled = undefined;
      }) as typeof clearTimeout,
      emitFailureNote: async () => {
        notes.push("failure");
      },
      onTimeout: (error) => {
        timeouts.push(error);
      },
    });

    watchdog.onAgentEvent({
      stream: "tool",
      data: {
        phase: "result",
        name: "exec",
        toolCallId: "tool-1",
        result: {
          content: [{ type: "text", text: "HTTP/2 504 upstream request timeout" }],
          details: { status: "completed" },
        },
      },
    });

    expect(scheduled?.delay).toBe(20_000);
    now += 20_000;
    scheduled?.fn();
    await Promise.resolve();

    expect(notes).toEqual(["failure"]);
    expect(timeouts).toHaveLength(1);
    expect(timeouts[0]?.message).toBe(POST_TOOL_CONTINUATION_TIMEOUT_ERROR_TEXT);
  });

  it("does not arm for running tool results", () => {
    let scheduled = false;

    const watchdog = createToolContinuationWatchdog({
      runId: "run-1",
      sessionId: "session-1",
      timeoutMs: 20_000,
      setTimer: ((fn: () => void, delay?: number) => {
        scheduled = true;
        return { fn, delay } as unknown as ReturnType<typeof setTimeout>;
      }) as typeof setTimeout,
      clearTimer: (() => {}) as typeof clearTimeout,
      emitFailureNote: async () => {},
      onTimeout: () => {},
    });

    watchdog.onAgentEvent({
      stream: "tool",
      data: {
        phase: "result",
        name: "exec",
        toolCallId: "tool-1",
        result: {
          content: [{ type: "text", text: "Command still running." }],
          details: { status: "running" },
        },
      },
    });

    expect(scheduled).toBe(false);
  });

  it("disarms when assistant activity resumes after a terminal tool result", () => {
    let scheduled: { fn: () => void; delay: number } | undefined;
    let clearCount = 0;

    const watchdog = createToolContinuationWatchdog({
      runId: "run-1",
      sessionId: "session-1",
      timeoutMs: 20_000,
      setTimer: ((fn: () => void, delay?: number) => {
        scheduled = { fn, delay: delay ?? 0 };
        return scheduled as unknown as ReturnType<typeof setTimeout>;
      }) as typeof setTimeout,
      clearTimer: (() => {
        clearCount += 1;
        scheduled = undefined;
      }) as typeof clearTimeout,
      emitFailureNote: async () => {},
      onTimeout: () => {
        throw new Error("watchdog should not have timed out");
      },
    });

    watchdog.onAgentEvent({
      stream: "tool",
      data: {
        phase: "result",
        name: "exec",
        toolCallId: "tool-1",
        result: {
          content: [{ type: "text", text: "status 200 ok" }],
          details: { status: "completed" },
        },
      },
    });

    expect(scheduled?.delay).toBe(20_000);

    watchdog.onAgentEvent({
      stream: "assistant",
      data: {
        phase: "message_update",
        text: "summary",
      },
    } satisfies ToolContinuationWatchdogAgentEvent);

    expect(clearCount).toBeGreaterThan(0);
    expect(scheduled).toBeUndefined();
  });
});
