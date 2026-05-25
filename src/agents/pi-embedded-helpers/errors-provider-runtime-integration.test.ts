import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { testing as providerRuntimeTesting } from "../../plugins/provider-runtime.js";
import { createEmptyPluginRegistry } from "../../plugins/registry.js";
import { resetPluginRuntimeStateForTest, setActivePluginRegistry } from "../../plugins/runtime.js";
import type { ProviderPlugin } from "../../plugins/types.js";
import { classifyFailoverSignal, classifyProviderRuntimeFailureKind } from "./errors.js";

function installProviderPlugin(provider: ProviderPlugin) {
  const registry = createEmptyPluginRegistry();
  registry.providers.push({
    pluginId: provider.id,
    pluginName: provider.label,
    provider,
    source: "integration-test",
  });
  setActivePluginRegistry(registry, `provider-error:${provider.id}`);
}

function resetRuntimePlugins() {
  resetPluginRuntimeStateForTest();
  providerRuntimeTesting.clearProviderRuntimePluginCacheForTest();
}

describe("classifyFailoverSignal provider runtime integration", () => {
  beforeEach(() => {
    resetRuntimePlugins();
  });

  afterEach(() => {
    resetRuntimePlugins();
  });

  it("uses provider plugin descriptors without bypassing status precedence", () => {
    installProviderPlugin({
      id: "xai",
      label: "xAI",
      auth: [],
      classifyProviderError: ({ code }) => {
        if (code === "SPENDING_LIMIT") {
          return {
            reason: "billing",
            code,
            userMessage: "Your xAI account has reached its spending limit.",
            action: {
              kind: "usage",
              label: "Review xAI usage",
              url: "https://grok.com/?_s=usage",
            },
          };
        }
        return code === "TEMPORARY_LIMIT" ? { reason: "rate_limit", code } : undefined;
      },
      classifyFailoverReason: ({ errorMessage }) =>
        /\bdeprecated temperature\b/i.test(errorMessage) ? "format" : undefined,
    });

    expect(
      classifyFailoverSignal({
        provider: "xai",
        status: 403,
        code: "SPENDING_LIMIT",
        message: "Forbidden",
      }),
    ).toEqual({ kind: "reason", reason: "billing" });
    expect(
      classifyFailoverSignal({
        provider: "xai",
        status: 429,
        code: "SPENDING_LIMIT",
        message: "Forbidden",
      }),
    ).toEqual({ kind: "reason", reason: "rate_limit" });
    expect(
      classifyFailoverSignal({
        provider: "xai",
        status: 403,
        code: "TEMPORARY_LIMIT",
        message: "Forbidden",
      }),
    ).toEqual({ kind: "reason", reason: "rate_limit" });
    expect(
      classifyFailoverSignal({
        provider: "xai",
        status: 403,
        code: "TEMPORARY_LIMIT",
        message: "account has been deactivated",
      }),
    ).toEqual({ kind: "reason", reason: "auth_permanent" });
    expect(
      classifyFailoverSignal({
        provider: "openai",
        status: 403,
        code: "SPENDING_LIMIT",
        message: "Forbidden",
      }),
    ).toEqual({ kind: "reason", reason: "auth" });
    expect(
      classifyFailoverSignal({
        provider: "xai",
        status: 402,
        code: "TEMPORARY_LIMIT",
        message: "Payment Required",
      }),
    ).toEqual({ kind: "reason", reason: "rate_limit" });
    expect(
      classifyProviderRuntimeFailureKind({
        provider: "xai",
        message: "ValidationException: deprecated temperature",
      }),
    ).toBe("schema");
  });
});
