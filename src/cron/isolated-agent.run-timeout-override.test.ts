import { describe, expect, it } from "vitest";
import { resolveCronRunTimeoutOverrideMs } from "./isolated-agent/run-timeout.js";

describe("resolveCronRunTimeoutOverrideMs", () => {
  // Regression: when a cron job's payload `timeoutSeconds` numerically equals
  // the configured agent default, `timeoutMs !== defaultTimeoutMs` collapses to
  // `false` in the embedded runner. The cron entry point must carry a separate
  // explicit-timeout signal so the LLM idle watchdog does not fall back to its
  // implicit 120s cap.
  it("preserves explicit payload timeoutSeconds even when it equals the agent default", () => {
    expect(resolveCronRunTimeoutOverrideMs(300)).toBe(300_000);
  });

  it("preserves explicit payload timeoutSeconds when it differs from the agent default", () => {
    expect(resolveCronRunTimeoutOverrideMs(30)).toBe(30_000);
  });

  it("ignores missing, invalid, or non-positive timeout values", () => {
    expect(resolveCronRunTimeoutOverrideMs(undefined)).toBeUndefined();
    expect(resolveCronRunTimeoutOverrideMs(0)).toBeUndefined();
    expect(resolveCronRunTimeoutOverrideMs(-1)).toBeUndefined();
    expect(resolveCronRunTimeoutOverrideMs(Number.NaN)).toBeUndefined();
    expect(resolveCronRunTimeoutOverrideMs("300")).toBeUndefined();
  });
});
