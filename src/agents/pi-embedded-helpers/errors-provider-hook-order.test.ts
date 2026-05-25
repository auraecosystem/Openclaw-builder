import { beforeEach, describe, expect, it, vi } from "vitest";

const providerErrorPatternMocks = vi.hoisted(() => ({
  classifyProviderPluginError: vi.fn(),
}));

vi.mock("./provider-error-patterns.js", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./provider-error-patterns.js")>();
  return {
    ...actual,
    classifyProviderPluginError: providerErrorPatternMocks.classifyProviderPluginError,
  };
});

import { classifyFailoverSignal } from "./errors.js";

describe("classifyFailoverSignal provider hook ordering", () => {
  beforeEach(() => {
    providerErrorPatternMocks.classifyProviderPluginError.mockReset();
  });

  it("feeds provider plugin classifications through HTTP status precedence", () => {
    providerErrorPatternMocks.classifyProviderPluginError.mockReturnValue("billing");

    expect(
      classifyFailoverSignal({
        provider: "xai",
        status: 429,
        code: "SPENDING_LIMIT",
        message: "Forbidden",
      }),
    ).toEqual({ kind: "reason", reason: "rate_limit" });
  });

  it("lets provider plugin classifications refine ambiguous auth statuses", () => {
    providerErrorPatternMocks.classifyProviderPluginError.mockReturnValue("billing");

    expect(
      classifyFailoverSignal({
        provider: "xai",
        status: 403,
        code: "SPENDING_LIMIT",
        message: "Forbidden",
      }),
    ).toEqual({ kind: "reason", reason: "billing" });
  });

  it("keeps direct failover codes ahead of provider plugin classifications", () => {
    providerErrorPatternMocks.classifyProviderPluginError.mockReturnValue("billing");

    expect(
      classifyFailoverSignal({
        provider: "demo",
        code: "RATE_LIMIT",
        message: "SPENDING_LIMIT",
      }),
    ).toEqual({ kind: "reason", reason: "rate_limit" });
  });

  it("does not ask provider plugins to override context overflow", () => {
    providerErrorPatternMocks.classifyProviderPluginError.mockReturnValue("billing");

    expect(
      classifyFailoverSignal({
        provider: "demo",
        message: "prompt is too long: 150000 tokens > 128000 maximum",
      }),
    ).toEqual({ kind: "context_overflow" });
    expect(providerErrorPatternMocks.classifyProviderPluginError).not.toHaveBeenCalled();
  });
});
