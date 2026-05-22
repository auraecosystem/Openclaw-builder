import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { makeAttemptResult } from "./run.overflow-compaction.fixture.js";
import {
  loadRunOverflowCompactionHarness,
  mockedClassifyFailoverReason,
  mockedGlobalHookRunner,
  mockedRunEmbeddedAttempt,
  overflowBaseRunParams,
  resetRunOverflowCompactionHarnessMocks,
} from "./run.overflow-compaction.harness.js";
import type { EmbeddedRunAttemptResult } from "./run/types.js";

let runEmbeddedPiAgent: typeof import("./run.js").runEmbeddedPiAgent;

function successAttempt(provider: string, model: string): EmbeddedRunAttemptResult {
  return makeAttemptResult({
    assistantTexts: ["done"],
    lastAssistant: {
      stopReason: "stop",
      provider,
      model,
      content: [{ type: "text", text: "done" }],
      usage: { input: 100, output: 5, totalTokens: 105 },
    } as unknown as EmbeddedRunAttemptResult["lastAssistant"],
  });
}

type FastModeAttemptParams = {
  fastMode?: unknown;
  onAgentEvent?: (event: { stream: string; data: Record<string, unknown> }) => unknown;
  onRunProgress?: (payload: { reason: string }) => unknown;
  onToolResult?: (payload: { text?: string; channelData?: Record<string, unknown> }) => unknown;
};

function resolveAttemptFastMode(params: unknown): void {
  const fastMode = (params as { fastMode?: unknown }).fastMode;
  if (typeof fastMode === "function") {
    fastMode();
  }
}

describe("runEmbeddedPiAgent fast auto progress", () => {
  beforeAll(async () => {
    ({ runEmbeddedPiAgent } = await loadRunOverflowCompactionHarness());
  });

  beforeEach(() => {
    resetRunOverflowCompactionHarnessMocks();
    mockedGlobalHookRunner.hasHooks.mockImplementation(() => false);
    mockedClassifyFailoverReason.mockReturnValue(null);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("emits auto-off after a tool execution boundary crosses the threshold", async () => {
    vi.useFakeTimers();

    const events: Array<{
      data?: { summary?: unknown };
    }> = [];
    const toolResults: Array<{
      text?: string;
      channelData?: Record<string, unknown>;
    }> = [];
    let attemptParams: FastModeAttemptParams | undefined;
    let completeAttempt: (() => void) | undefined;
    const attemptDone = new Promise<EmbeddedRunAttemptResult>((resolve) => {
      completeAttempt = () => {
        resolve(successAttempt("ollama", "glm-5.1:cloud"));
      };
    });
    mockedRunEmbeddedAttempt.mockImplementationOnce(async (params) => {
      attemptParams = params as FastModeAttemptParams;
      resolveAttemptFastMode(params);
      return attemptDone;
    });

    const resultPromise = runEmbeddedPiAgent({
      ...overflowBaseRunParams,
      provider: "ollama",
      model: "glm-5.1:cloud",
      runId: "run-fast-auto-retry",
      fastMode: "auto",
      fastModeAutoSeconds: 1,
      onAgentEvent: (event) => {
        events.push(event);
      },
      onToolResult: (payload) => {
        toolResults.push(payload);
      },
    });

    await vi.waitFor(() => {
      expect(mockedRunEmbeddedAttempt).toHaveBeenCalledTimes(1);
    });
    await vi.advanceTimersByTimeAsync(1100);

    expect(events.map((event) => event.data?.summary).filter(Boolean)).toHaveLength(0);
    expect(toolResults).toHaveLength(0);

    attemptParams?.onRunProgress?.({ reason: "model-progress" });
    await vi.advanceTimersByTimeAsync(2);
    expect(events).toHaveLength(0);
    expect(toolResults).toHaveLength(0);

    await attemptParams?.onAgentEvent?.({
      stream: "tool",
      data: { phase: "start", name: "exec" },
    });
    await vi.advanceTimersByTimeAsync(2);

    expect(events.map((event) => event.data?.summary).filter(Boolean)).toHaveLength(0);
    expect(toolResults).toHaveLength(0);

    await attemptParams?.onAgentEvent?.({
      stream: "tool",
      data: { phase: "result", name: "exec" },
    });
    await vi.advanceTimersByTimeAsync(2);

    const summaries = events.map((event) => event.data?.summary).filter(Boolean);
    expect(summaries.some((summary) => String(summary).startsWith("💨Fast: auto-off("))).toBe(true);
    expect(toolResults.some((payload) => payload.text?.startsWith("💨Fast: auto-off("))).toBe(true);
    expect(toolResults.at(-1)?.channelData?.openclawProgressKind).toBe("fast-mode-auto");

    completeAttempt?.();
    await resultPromise;

    expect(events.map((event) => event.data?.summary)).toContain("💨Fast: auto-on");
    expect(toolResults.map((payload) => payload.text)).toContain("💨Fast: auto-on");
  });
});
