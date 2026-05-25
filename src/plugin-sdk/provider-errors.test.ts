import { describe, expect, it } from "vitest";
import {
  classifyProviderErrorFromMap,
  defineProviderErrorMap,
  normalizeProviderErrorCodeSignal,
} from "./provider-errors.js";

describe("provider error map helpers", () => {
  it("normalizes provider error codes", () => {
    expect(normalizeProviderErrorCodeSignal(" spending-limit ")).toBe("SPENDING_LIMIT");
    expect(normalizeProviderErrorCodeSignal("rate.limit/reached")).toBe("RATE_LIMIT_REACHED");
    expect(normalizeProviderErrorCodeSignal(" ")).toBeUndefined();
  });

  it("maps direct provider codes to structured descriptors", () => {
    const classifyProviderError = defineProviderErrorMap([
      {
        codes: ["SPENDING_LIMIT"],
        reason: "billing",
        userMessage: "Your xAI account has reached its spending limit.",
        action: {
          kind: "usage",
          label: "Review usage",
          url: "https://grok.com/?_s=usage",
        },
      },
    ]);

    expect(
      classifyProviderError({
        provider: "xai",
        errorMessage: "Forbidden",
        status: 403,
        code: "SPENDING_LIMIT",
      }),
    ).toEqual({
      reason: "billing",
      status: 403,
      code: "SPENDING_LIMIT",
      userMessage: "Your xAI account has reached its spending limit.",
      action: {
        kind: "usage",
        label: "Review usage",
        url: "https://grok.com/?_s=usage",
      },
    });
  });

  it("detects provider codes embedded in JSON error text without substring matches", () => {
    const entries = [
      {
        codes: ["SPENDING_LIMIT"],
        reason: "billing",
      },
    ] as const;

    expect(
      classifyProviderErrorFromMap(
        {
          errorMessage: '{"error":{"code":"SPENDING_LIMIT","message":"Usage cap reached"}}',
        },
        entries,
      )?.reason,
    ).toBe("billing");
    expect(
      classifyProviderErrorFromMap(
        {
          errorMessage: '{"error":{"code":"SPENDING_LIMITED","message":"Different code"}}',
        },
        entries,
      ),
    ).toBeUndefined();
    expect(
      classifyProviderErrorFromMap(
        {
          errorMessage: "This is not a SPENDING_LIMIT problem.",
        },
        entries,
      ),
    ).toBeUndefined();
    expect(
      classifyProviderErrorFromMap(
        {
          errorMessage: '{"error":{"code":"SPENDING_LIMIT","message":"Usage cap reached"}}',
          code: "ERR_BAD_REQUEST",
        },
        entries,
      ),
    ).toEqual({
      reason: "billing",
      code: "SPENDING_LIMIT",
    });
  });

  it("treats direct code and type mismatches as authoritative", () => {
    const classifyProviderError = defineProviderErrorMap([
      {
        codes: ["SPENDING_LIMIT"],
        reason: "billing",
      },
    ]);

    expect(
      classifyProviderError({
        errorMessage: "This is not a SPENDING_LIMIT problem.",
        code: "INSUFFICIENT_SCOPE",
      }),
    ).toBeUndefined();
    expect(
      classifyProviderError({
        errorMessage: "This is not a SPENDING_LIMIT problem.",
        errorType: "INSUFFICIENT_SCOPE",
      }),
    ).toBeUndefined();
  });

  it("supports status, message, and computed recovery values", () => {
    const classifyProviderError = defineProviderErrorMap([
      {
        status: 429,
        messagePatterns: [/try again/i],
        reason: "rate_limit",
        retryAfterMs: ({ status }) => (status === 429 ? 30_000 : undefined),
      },
    ]);

    expect(
      classifyProviderError({
        errorMessage: "Please try again later.",
        status: 429,
      }),
    ).toEqual({
      reason: "rate_limit",
      status: 429,
      retryAfterMs: 30_000,
    });
    expect(
      classifyProviderError({
        errorMessage: "Please try again later.",
        status: 503,
      }),
    ).toBeUndefined();
  });
});
