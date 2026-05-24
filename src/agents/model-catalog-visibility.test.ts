import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OpenClawConfig } from "../config/types.openclaw.js";
import { resolveVisibleModelCatalog } from "./model-catalog-visibility.js";
import type { ModelCatalogEntry } from "./model-catalog.types.js";

function createProviderAuthChecker(predicate: (provider: string) => boolean): {
  calls: string[];
  check: (provider: string) => Promise<boolean>;
} {
  const calls: string[] = [];
  return {
    calls,
    check: async (provider: string) => {
      calls.push(provider);
      return predicate(provider);
    },
  };
}

describe("resolveVisibleModelCatalog", () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it("can use static auth checks for gateway read-only model lists", async () => {
    const authChecker = createProviderAuthChecker((provider) => provider === "openai");
    const catalog: ModelCatalogEntry[] = [
      { provider: "anthropic", id: "claude-test", name: "Claude Test" },
      { provider: "openai", id: "gpt-test", name: "GPT Test" },
    ];
    const cfg = {} as OpenClawConfig;

    const result = await resolveVisibleModelCatalog({
      cfg,
      catalog,
      defaultProvider: "openai",
      runtimeAuthDiscovery: false,
      providerAuthChecker: authChecker.check,
    });

    expect(authChecker.calls).toEqual(["anthropic", "openai"]);
    expect(result).toEqual([{ provider: "openai", id: "gpt-test", name: "GPT Test" }]);
  });

  it("limits visible catalog to provider wildcard entries after default discovery", async () => {
    const authChecker = createProviderAuthChecker((provider) => provider !== "blocked");
    const catalog: ModelCatalogEntry[] = [
      { provider: "anthropic", id: "claude-test", name: "Claude Test" },
      { provider: "openai-codex", id: "gpt-codex-test", name: "GPT Codex Test" },
      { provider: "vllm", id: "qwen-local", name: "Qwen Local" },
      { provider: "blocked", id: "blocked-test", name: "Blocked Test" },
    ];

    const cfg = {
      agents: {
        defaults: {
          models: {
            "vllm/*": {},
            "openai-codex/*": {},
            "blocked/*": {},
          },
        },
      },
    } as OpenClawConfig;

    const result = await resolveVisibleModelCatalog({
      cfg,
      catalog,
      defaultProvider: "anthropic",
      runtimeAuthDiscovery: true,
      providerAuthChecker: authChecker.check,
    });

    expect(authChecker.calls).toEqual(["anthropic", "openai-codex", "vllm", "blocked"]);
    expect(result).toEqual([
      { provider: "openai-codex", id: "gpt-codex-test", name: "GPT Codex Test" },
      { provider: "vllm", id: "qwen-local", name: "Qwen Local" },
    ]);
  });

  it("does not broaden visibility when selected providers have no catalog rows", async () => {
    const authChecker = createProviderAuthChecker(() => true);

    const cfg = {
      agents: {
        defaults: {
          models: {
            "vllm/*": {},
          },
        },
      },
    } as OpenClawConfig;

    const result = await resolveVisibleModelCatalog({
      cfg,
      catalog: [{ provider: "anthropic", id: "claude-test", name: "Claude Test" }],
      defaultProvider: "anthropic",
      runtimeAuthDiscovery: true,
      providerAuthChecker: authChecker.check,
    });

    expect(authChecker.calls).toEqual(["anthropic"]);
    expect(result).toEqual([]);
  });
});
