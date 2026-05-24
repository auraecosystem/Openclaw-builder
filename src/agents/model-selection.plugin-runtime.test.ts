import { beforeEach, describe, expect, it, vi } from "vitest";

const normalizeProviderModelIdWithPluginMock = vi.fn();
const emptyPluginMetadataSnapshot = vi.hoisted(() => ({
  configFingerprint: "model-selection-plugin-runtime-test-empty-plugin-metadata",
  plugins: [
    {
      modelIdNormalization: {
        providers: {
          google: {
            aliases: {
              "gemini-3.1-pro": "gemini-3.1-pro-preview",
            },
          },
        },
      },
    },
  ],
}));

vi.mock("./provider-model-normalization.runtime.js", () => ({
  normalizeProviderModelIdWithRuntime: (params: unknown) =>
    normalizeProviderModelIdWithPluginMock(params),
}));

vi.mock("../plugins/current-plugin-metadata-snapshot.js", () => ({
  getCurrentPluginMetadataSnapshot: () => emptyPluginMetadataSnapshot,
}));

describe("model-selection plugin runtime normalization", () => {
  beforeEach(() => {
    vi.resetModules();
    normalizeProviderModelIdWithPluginMock.mockReset();
  });

  it("delegates provider-owned model id normalization to plugin runtime hooks", async () => {
    normalizeProviderModelIdWithPluginMock.mockImplementation(({ provider, context }) => {
      if (
        provider === "custom-provider" &&
        (context as { modelId?: string }).modelId === "custom-legacy-model"
      ) {
        return "custom-modern-model";
      }
      return undefined;
    });

    const { parseModelRef } = await import("./model-selection.js");

    expect(parseModelRef("custom-legacy-model", "custom-provider")).toEqual({
      provider: "custom-provider",
      model: "custom-modern-model",
    });
    expect(normalizeProviderModelIdWithPluginMock).toHaveBeenCalledWith({
      provider: "custom-provider",
      context: {
        provider: "custom-provider",
        modelId: "custom-legacy-model",
      },
    });
  });

  it("keeps static normalization while skipping plugin runtime hooks when disabled", async () => {
    const { parseModelRef } = await import("./model-selection.js");

    expect(
      parseModelRef("gemini-3.1-pro", "google", {
        allowPluginNormalization: false,
      }),
    ).toEqual({
      provider: "google",
      model: "gemini-3.1-pro-preview",
    });
    expect(normalizeProviderModelIdWithPluginMock).not.toHaveBeenCalled();
  });

  it("keeps model visibility policy construction off plugin runtime hooks by default", async () => {
    normalizeProviderModelIdWithPluginMock.mockImplementation(({ provider, context }) => {
      if (
        provider === "custom-provider" &&
        (context as { modelId?: string }).modelId === "custom-legacy-model"
      ) {
        return "custom-modern-model";
      }
      return undefined;
    });

    const { createModelVisibilityPolicy } = await import("./model-visibility-policy.js");

    const policy = createModelVisibilityPolicy({
      cfg: {
        agents: {
          defaults: {
            models: {
              "custom-provider/custom-legacy-model": {},
            },
          },
        },
      },
      catalog: [],
      defaultProvider: "custom-provider",
      defaultModel: "custom-legacy-model",
    });

    expect(policy.allowedKeys.has("custom-provider/custom-legacy-model")).toBe(true);
    expect(policy.allowedKeys.has("custom-provider/custom-modern-model")).toBe(false);
    expect(normalizeProviderModelIdWithPluginMock).not.toHaveBeenCalled();
  });

  it("propagates explicit plugin runtime normalization opt-in through model visibility policy", async () => {
    normalizeProviderModelIdWithPluginMock.mockImplementation(({ provider, context }) => {
      if (
        provider === "custom-provider" &&
        (context as { modelId?: string }).modelId === "custom-legacy-model"
      ) {
        return "custom-modern-model";
      }
      return undefined;
    });

    const { createModelVisibilityPolicy } = await import("./model-visibility-policy.js");

    const policy = createModelVisibilityPolicy({
      cfg: {
        agents: {
          defaults: {
            models: {
              "custom-provider/custom-legacy-model": {},
            },
          },
        },
      },
      catalog: [],
      defaultProvider: "custom-provider",
      defaultModel: "custom-legacy-model",
      allowPluginNormalization: true,
    });

    expect(policy.allowedKeys.has("custom-provider/custom-modern-model")).toBe(true);
    expect(normalizeProviderModelIdWithPluginMock).toHaveBeenCalled();
  });
});
