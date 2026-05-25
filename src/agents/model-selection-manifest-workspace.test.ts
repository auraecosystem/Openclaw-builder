import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OpenClawConfig } from "../config/types.openclaw.js";

const loadManifestMetadataSnapshotMock = vi.hoisted(() => vi.fn());
const getCurrentPluginMetadataSnapshotMock = vi.hoisted(() => vi.fn());
const getActivePluginRegistryWorkspaceDirFromStateMock = vi.hoisted(() => vi.fn());

vi.mock("../plugins/current-plugin-metadata-snapshot.js", () => ({
  getCurrentPluginMetadataSnapshot: getCurrentPluginMetadataSnapshotMock,
}));

vi.mock("../plugins/manifest-contract-eligibility.js", () => ({
  loadManifestMetadataSnapshot: loadManifestMetadataSnapshotMock,
}));

vi.mock("../plugins/runtime-state.js", () => ({
  getActivePluginRegistryWorkspaceDirFromState: getActivePluginRegistryWorkspaceDirFromStateMock,
}));

vi.mock("./provider-model-normalization.runtime.js", () => ({
  normalizeProviderModelIdWithRuntime: () => undefined,
}));

describe("configured model manifest workspace scope", () => {
  beforeEach(() => {
    vi.resetModules();
    loadManifestMetadataSnapshotMock.mockReset();
    getCurrentPluginMetadataSnapshotMock.mockReset();
    getActivePluginRegistryWorkspaceDirFromStateMock.mockReset();
    getCurrentPluginMetadataSnapshotMock.mockReturnValue(undefined);
    loadManifestMetadataSnapshotMock.mockReturnValue({
      plugins: [
        {
          modelIdNormalization: {
            providers: {
              custom: {
                prefixWhenBare: "workspace-custom",
              },
            },
          },
        },
      ],
    });
  });

  it("does not reuse workspace manifest policies without a workspace context", async () => {
    const { buildConfiguredModelCatalog } = await import("./model-selection-shared.js");
    const cfg = {
      models: {
        providers: {
          custom: {
            models: [{ id: "fast-model" }],
          },
        },
      },
    } as unknown as OpenClawConfig;

    expect(buildConfiguredModelCatalog({ cfg })).toMatchObject([
      {
        provider: "custom",
        id: "fast-model",
      },
    ]);
    expect(getCurrentPluginMetadataSnapshotMock).toHaveBeenCalledWith({
      config: cfg,
      env: process.env,
    });
    expect(loadManifestMetadataSnapshotMock).not.toHaveBeenCalled();
  });

  it("uses manifest policies when the workspace context is explicit", async () => {
    const { buildConfiguredModelCatalog } = await import("./model-selection-shared.js");
    const cfg = {
      models: {
        providers: {
          custom: {
            models: [{ id: "fast-model" }],
          },
        },
      },
    } as unknown as OpenClawConfig;

    expect(buildConfiguredModelCatalog({ cfg, workspaceDir: "/workspace/a" })).toMatchObject([
      {
        provider: "custom",
        id: "workspace-custom/fast-model",
      },
    ]);
    expect(loadManifestMetadataSnapshotMock).toHaveBeenCalledWith({
      config: cfg,
      workspaceDir: "/workspace/a",
      env: process.env,
    });
    expect(getCurrentPluginMetadataSnapshotMock).not.toHaveBeenCalled();
  });

  it("uses an unscoped current snapshot without falling back to a metadata scan", async () => {
    getCurrentPluginMetadataSnapshotMock.mockReturnValue({
      plugins: [
        {
          modelIdNormalization: {
            providers: {
              custom: {
                prefixWhenBare: "global-custom",
              },
            },
          },
        },
      ],
    });
    const { buildConfiguredModelCatalog } = await import("./model-selection-shared.js");
    const cfg = {
      models: {
        providers: {
          custom: {
            models: [{ id: "fast-model" }],
          },
        },
      },
    } as unknown as OpenClawConfig;

    expect(buildConfiguredModelCatalog({ cfg })).toMatchObject([
      {
        provider: "custom",
        id: "global-custom/fast-model",
      },
    ]);
    expect(loadManifestMetadataSnapshotMock).not.toHaveBeenCalled();
  });

  it("does not load manifest metadata for empty configured model aliases", async () => {
    const { buildModelAliasIndex } = await import("./model-selection-shared.js");
    const cfg = {} as unknown as OpenClawConfig;

    const aliases = buildModelAliasIndex({ cfg, defaultProvider: "anthropic" });

    expect(aliases.byAlias.size).toBe(0);
    expect(aliases.byKey.size).toBe(0);
    expect(getCurrentPluginMetadataSnapshotMock).not.toHaveBeenCalled();
    expect(loadManifestMetadataSnapshotMock).not.toHaveBeenCalled();
  });

  it("does not load manifest metadata for wildcard-only configured model aliases", async () => {
    const { buildModelAliasIndex } = await import("./model-selection-shared.js");
    const cfg = {
      agents: {
        defaults: {
          models: {
            "anthropic/*": {},
          },
        },
      },
    } as unknown as OpenClawConfig;

    const aliases = buildModelAliasIndex({ cfg, defaultProvider: "anthropic" });

    expect(aliases.byAlias.size).toBe(0);
    expect(aliases.byKey.size).toBe(0);
    expect(getCurrentPluginMetadataSnapshotMock).not.toHaveBeenCalled();
    expect(loadManifestMetadataSnapshotMock).not.toHaveBeenCalled();
  });

  it("does not load manifest metadata for configured model entries without aliases", async () => {
    const { buildModelAliasIndex } = await import("./model-selection-shared.js");
    const cfg = {
      agents: {
        defaults: {
          models: {
            "anthropic/sonnet-4.6": {},
          },
        },
      },
    } as unknown as OpenClawConfig;

    const aliases = buildModelAliasIndex({ cfg, defaultProvider: "anthropic" });

    expect(aliases.byAlias.size).toBe(0);
    expect(aliases.byKey.size).toBe(0);
    expect(getCurrentPluginMetadataSnapshotMock).not.toHaveBeenCalled();
    expect(loadManifestMetadataSnapshotMock).not.toHaveBeenCalled();
  });

  it("reuses resolved manifest plugins while resolving configured model aliases", async () => {
    loadManifestMetadataSnapshotMock.mockReturnValue({
      plugins: [
        {
          modelIdNormalization: {
            providers: {
              anthropic: {
                aliases: {
                  "sonnet-4.6": "claude-sonnet-4-6",
                },
              },
              openrouter: {
                prefixWhenBare: "openrouter",
              },
            },
          },
        },
      ],
    });
    const { resolveConfiguredModelRef } = await import("./model-selection-shared.js");
    const cfg = {
      agents: {
        defaults: {
          model: { primary: "router-auto" },
          models: {
            "anthropic/sonnet-4.6": { alias: "sonnet" },
            "openrouter:auto": { alias: "router-auto" },
          },
        },
      },
    } as unknown as OpenClawConfig;

    expect(
      resolveConfiguredModelRef({
        cfg,
        defaultProvider: "anthropic",
        defaultModel: "claude-sonnet-4-6",
      }),
    ).toEqual({ provider: "openrouter", model: "openrouter/auto" });
    expect(loadManifestMetadataSnapshotMock).toHaveBeenCalledTimes(1);
  });
});
