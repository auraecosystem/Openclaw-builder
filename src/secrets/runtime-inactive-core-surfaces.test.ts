import { describe, expect, it } from "vitest";
import { asConfig, setupSecretsRuntimeSnapshotTestHooks } from "./runtime.test-support.ts";

const { prepareSecretsRuntimeSnapshot } = setupSecretsRuntimeSnapshotTestHooks();

function expectWarningPaths(
  snapshot: Awaited<ReturnType<typeof prepareSecretsRuntimeSnapshot>>,
  expectedPaths: string[],
): void {
  const warningPaths = new Set(snapshot.warnings.map((warning) => warning.path));
  for (const expectedPath of expectedPaths) {
    expect(warningPaths.has(expectedPath)).toBe(true);
  }
}

describe("secrets runtime snapshot inactive core surfaces", () => {
  it("skips inactive core refs and emits diagnostics", async () => {
    const snapshot = await prepareSecretsRuntimeSnapshot({
      config: asConfig({
        agents: {
          defaults: {
            memorySearch: {
              enabled: false,
              remote: {
                apiKey: { source: "env", provider: "default", id: "DISABLED_MEMORY_API_KEY" },
              },
            },
          },
        },
        gateway: {
          auth: {
            mode: "token",
            password: { source: "env", provider: "default", id: "DISABLED_GATEWAY_PASSWORD" },
          },
        },
      }),
      env: {},
      includeAuthStoreRefs: false,
      loadablePluginOrigins: new Map(),
    });

    expectWarningPaths(snapshot, [
      "agents.defaults.memorySearch.remote.apiKey",
      "gateway.auth.password",
    ]);
  });

  it("skips inactive MCP transport refs and resolves only the active transport", async () => {
    const snapshot = await prepareSecretsRuntimeSnapshot({
      config: asConfig({
        mcp: {
          servers: {
            "mission-control": {
              command: "node",
              args: ["scripts/mc-mcp-server.cjs"],
              url: "https://example.invalid/mcp",
              env: {
                MC_API_KEY: { source: "env", provider: "default", id: "MC_API_KEY" },
              },
              headers: {
                Authorization: {
                  source: "env",
                  provider: "default",
                  id: "UNUSED_REMOTE_MCP_AUTH",
                },
              },
            },
          },
        },
      }),
      env: {
        MC_API_KEY: "mc-secret-runtime",
      },
      includeAuthStoreRefs: false,
      loadablePluginOrigins: new Map(),
    });

    expect(snapshot.config.mcp?.servers?.["mission-control"]?.env?.MC_API_KEY).toBe(
      "mc-secret-runtime",
    );
    expect(snapshot.config.mcp?.servers?.["mission-control"]?.headers?.Authorization).toEqual({
      source: "env",
      provider: "default",
      id: "UNUSED_REMOTE_MCP_AUTH",
    });
    expect(snapshot.warnings.map((warning) => warning.path)).toEqual(
      expect.arrayContaining(["mcp.servers.mission-control.headers.Authorization"]),
    );
  });

  it("treats unsupported MCP transports as inactive secret surfaces", async () => {
    const snapshot = await prepareSecretsRuntimeSnapshot({
      config: asConfig({
        mcp: {
          servers: {
            remote: {
              transport: "ws",
              url: "https://example.invalid/mcp",
              headers: {
                Authorization: {
                  source: "env",
                  provider: "default",
                  id: "UNSUPPORTED_REMOTE_MCP_AUTH",
                },
              },
            },
          },
        },
      }),
      env: {},
      includeAuthStoreRefs: false,
      loadablePluginOrigins: new Map(),
    });

    expect(snapshot.config.mcp?.servers?.remote?.headers?.Authorization).toEqual({
      source: "env",
      provider: "default",
      id: "UNSUPPORTED_REMOTE_MCP_AUTH",
    });
    expect(snapshot.warnings.map((warning) => warning.path)).toEqual(
      expect.arrayContaining(["mcp.servers.remote.headers.Authorization"]),
    );
  });
});
