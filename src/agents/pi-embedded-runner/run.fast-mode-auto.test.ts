import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { makeAttemptResult } from "./run.overflow-compaction.fixture.js";
import {
  loadRunOverflowCompactionHarness,
  mockedGlobalHookRunner,
  mockedRunEmbeddedAttempt,
  overflowBaseRunParams,
  resetRunOverflowCompactionHarnessMocks,
} from "./run.overflow-compaction.harness.js";
import type { EmbeddedRunAttemptResult } from "./run/types.js";

let runEmbeddedPiAgent: typeof import("./run.js").runEmbeddedPiAgent;

describe("runEmbeddedPiAgent fast auto progress", () => {
  beforeAll(async () => {
    ({ runEmbeddedPiAgent } = await loadRunOverflowCompactionHarness());
  });

  beforeEach(() => {
    resetRunOverflowCompactionHarnessMocks();
    mockedGlobalHookRunner.hasHooks.mockImplementation(() => false);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("emits auto-off on elapsed time even when the backend does not re-read fastMode", async () => {
    vi.useFakeTimers();

    const events: Array<{
      data?: { summary?: unknown };
    }> = [];
    let completeAttempt: (() => void) | undefined;
    const attemptDone = new Promise<EmbeddedRunAttemptResult>((resolve) => {
      completeAttempt = () => {
        resolve(
          makeAttemptResult({
            assistantTexts: ["done"],
          }),
        );
      };
    });
    mockedRunEmbeddedAttempt.mockImplementationOnce(async () => attemptDone);

    const resultPromise = runEmbeddedPiAgent({
      ...overflowBaseRunParams,
      runId: "run-fast-auto-timer",
      fastMode: "auto",
      fastModeAutoSeconds: 1,
      onAgentEvent: (event) => {
        events.push(event);
      },
    });

    await vi.waitFor(() => {
      expect(mockedRunEmbeddedAttempt).toHaveBeenCalledTimes(1);
    });
    await vi.advanceTimersByTimeAsync(1100);

    const summaries = events.map((event) => event.data?.summary).filter(Boolean);
    expect(summaries.some((summary) => String(summary).startsWith("💨Fast: auto-off("))).toBe(true);

    completeAttempt?.();
    await resultPromise;

    expect(events.map((event) => event.data?.summary)).toContain("💨Fast: auto-on");
  });
});
