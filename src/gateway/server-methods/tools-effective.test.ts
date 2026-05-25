import { beforeEach, describe, expect, it, vi } from "vitest";
import type { McpToolCatalog, SessionMcpRuntime } from "../../agents/pi-bundle-mcp-types.js";
import { setPluginToolMeta } from "../../plugins/tools.js";
import { ErrorCodes } from "../protocol/index.js";
import { testing, toolsEffectiveHandlers } from "./tools-effective.js";

const runtimeMocks = vi.hoisted(() => ({
  deliveryContextFromSession: vi.fn(() => ({
    channel: "telegram",
    to: "channel-1",
    accountId: "acct-1",
    threadId: "thread-2",
  })),
  applyFinalEffectiveToolPolicy: vi.fn(
    (params: { bundledTools: unknown[] }) => params.bundledTools,
  ),
  buildBundleMcpToolsFromCatalog: vi.fn(() => [] as unknown[]),
  getOrCreateSessionMcpRuntime: vi.fn(async () => ({ sessionId: "session-1" })),
  listAgentIds: vi.fn(() => ["main"]),
  getRuntimeConfig: vi.fn(() => ({})),
  loadSessionEntry: vi.fn(() => ({
    cfg: {},
    canonicalKey: "main:abc",
    entry: {
      sessionId: "session-1",
      updatedAt: 1,
      lastChannel: "telegram",
      lastAccountId: "acct-1",
      lastThreadId: "thread-2",
      lastTo: "channel-1",
      groupId: "group-4",
      groupChannel: "#ops",
      space: "workspace-5",
      chatType: "group",
      modelProvider: "openai",
      model: "gpt-4.1",
      spawnedBy: "agent:main:telegram:group:parent-group",
    },
  })),
  materializeBundleMcpToolsForRun: vi.fn(async () => ({
    tools: [] as unknown[],
    dispose: vi.fn(async () => undefined),
  })),
  peekSessionMcpRuntime: vi.fn<
    () => Pick<SessionMcpRuntime, "configFingerprint" | "peekCatalog"> | undefined
  >(() => undefined),
  resolveSessionMcpConfigSummary: vi.fn(() => ({
    fingerprint: "mcp:1:test",
    serverNames: [] as string[],
  })),
  resolveAgentWorkspaceDir: vi.fn(() => "/tmp/workspace-main"),
  resolveEffectiveToolInventory: vi.fn(() => ({
    agentId: "main",
    profile: "coding",
    groups: [
      {
        id: "core",
        label: "Built-in tools",
        source: "core",
        tools: [
          {
            id: "exec",
            label: "Exec",
            description: "Run shell commands",
            rawDescription: "Run shell commands",
            source: "core",
          },
        ],
      },
    ],
  })),
  resolveReplyToMode: vi.fn(() => "first"),
  resolveSessionAgentId: vi.fn(() => "main"),
  resolveSessionModelRef: vi.fn(() => ({ provider: "openai", model: "gpt-4.1" })),
}));

vi.mock("./tools-effective.runtime.js", () => runtimeMocks);

type RespondCall = [boolean, unknown?, { code: number; message: string }?];
type ToolsEffectivePayload = {
  agentId?: string;
  profile?: string;
  notices?: Array<{ id?: string; severity?: string; message?: string }>;
  groups?: Array<{
    id?: string;
    label?: string;
    source?: string;
    tools?: Array<{ id?: string; label?: string; source?: string; pluginId?: string }>;
  }>;
};

function createInvokeParams(
  params: Record<string, unknown>,
  method: "tools.effective" | "tools.effective.refresh" = "tools.effective",
) {
  const respond = vi.fn();
  return {
    respond,
    invoke: async () =>
      await toolsEffectiveHandlers[method]({
        params,
        respond: respond as never,
        context: { getRuntimeConfig: () => ({}) } as never,
        client: null,
        req: { type: "req", id: "req-1", method },
        isWebchatConnect: () => false,
      }),
  };
}

function resolveEffectiveToolInventoryArg(callIndex = 0): Record<string, unknown> | undefined {
  const calls = runtimeMocks.resolveEffectiveToolInventory.mock.calls as unknown as Array<
    [Record<string, unknown>]
  >;
  return calls[callIndex]?.[0];
}

function firstRespondCall(respond: ReturnType<typeof vi.fn>): RespondCall | undefined {
  return respond.mock.calls[0] as RespondCall | undefined;
}

function makeMcpTool() {
  const mcpTool = {
    name: "reproProbe__probe_tool",
    label: "Probe Tool",
    description: "Probe from MCP",
    parameters: { type: "object", properties: {} },
    execute: vi.fn(),
  };
  setPluginToolMeta(mcpTool as never, { pluginId: "bundle-mcp", optional: false });
  return mcpTool;
}

describe("tools.effective handler", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    testing.resetToolsEffectiveCacheForTest();
    testing.resetToolsEffectiveNowForTest();
    runtimeMocks.resolveAgentWorkspaceDir.mockReturnValue("/tmp/workspace-main");
    runtimeMocks.resolveSessionMcpConfigSummary.mockReturnValue({
      fingerprint: "mcp:1:test",
      serverNames: [] as string[],
    });
    runtimeMocks.peekSessionMcpRuntime.mockReturnValue(undefined);
    runtimeMocks.getOrCreateSessionMcpRuntime.mockResolvedValue({ sessionId: "session-1" });
    runtimeMocks.materializeBundleMcpToolsForRun.mockResolvedValue({
      tools: [] as unknown[],
      dispose: vi.fn(async () => undefined),
    });
    runtimeMocks.buildBundleMcpToolsFromCatalog.mockReturnValue([]);
    runtimeMocks.applyFinalEffectiveToolPolicy.mockImplementation(
      (params: { bundledTools: unknown[] }) => params.bundledTools,
    );
  });

  it("rejects invalid params", async () => {
    const { respond, invoke } = createInvokeParams({ includePlugins: false });
    await invoke();
    const call = firstRespondCall(respond);
    expect(call?.[0]).toBe(false);
    expect(call?.[2]?.code).toBe(ErrorCodes.INVALID_REQUEST);
    expect(call?.[2]?.message).toContain("invalid tools.effective params");
  });

  it("rejects missing sessionKey", async () => {
    const { respond, invoke } = createInvokeParams({});
    await invoke();
    const call = firstRespondCall(respond);
    expect(call?.[0]).toBe(false);
    expect(call?.[2]?.code).toBe(ErrorCodes.INVALID_REQUEST);
    expect(call?.[2]?.message).toContain("invalid tools.effective params");
  });

  it("rejects caller-supplied auth context params", async () => {
    const { respond, invoke } = createInvokeParams({ senderIsOwner: true });
    await invoke();
    const call = firstRespondCall(respond);
    expect(call?.[0]).toBe(false);
    expect(call?.[2]?.code).toBe(ErrorCodes.INVALID_REQUEST);
    expect(call?.[2]?.message).toContain("invalid tools.effective params");
  });

  it("rejects unknown agent ids", async () => {
    const { respond, invoke } = createInvokeParams({
      sessionKey: "main:abc",
      agentId: "unknown-agent",
    });
    await invoke();
    const call = firstRespondCall(respond);
    expect(call?.[0]).toBe(false);
    expect(call?.[2]?.code).toBe(ErrorCodes.INVALID_REQUEST);
    expect(call?.[2]?.message).toContain("unknown agent id");
  });

  it("rejects unknown session keys", async () => {
    runtimeMocks.loadSessionEntry.mockReturnValueOnce({
      cfg: {},
      canonicalKey: "missing-session",
      entry: undefined,
      legacyKey: undefined,
      storePath: "/tmp/sessions.json",
    } as never);
    const { respond, invoke } = createInvokeParams({ sessionKey: "missing-session" });
    await invoke();
    const call = firstRespondCall(respond);
    expect(call?.[0]).toBe(false);
    expect(call?.[2]?.code).toBe(ErrorCodes.INVALID_REQUEST);
    expect(call?.[2]?.message).toContain('unknown session key "missing-session"');
  });

  it("returns the read-only effective runtime inventory without MCP startup", async () => {
    const { respond, invoke } = createInvokeParams({ sessionKey: "main:abc" });
    await invoke();
    const call = firstRespondCall(respond);
    expect(call?.[0]).toBe(true);
    const payload = call?.[1] as ToolsEffectivePayload | undefined;
    expect(payload?.agentId).toBe("main");
    expect(payload?.profile).toBe("coding");
    expect(payload?.groups?.[0]?.id).toBe("core");
    expect(payload?.groups?.[0]?.source).toBe("core");
    expect(payload?.groups?.[0]?.tools?.[0]?.id).toBe("exec");
    expect(runtimeMocks.getOrCreateSessionMcpRuntime).not.toHaveBeenCalled();
    expect(runtimeMocks.materializeBundleMcpToolsForRun).not.toHaveBeenCalled();
    const inventoryParams = resolveEffectiveToolInventoryArg();
    expect(inventoryParams?.currentChannelId).toBe("channel-1");
    expect(inventoryParams?.currentThreadTs).toBe("thread-2");
    expect(inventoryParams?.accountId).toBe("acct-1");
    expect(inventoryParams?.groupId).toBe("group-4");
    expect(inventoryParams?.groupChannel).toBe("#ops");
    expect(inventoryParams?.groupSpace).toBe("workspace-5");
    expect(inventoryParams?.replyToMode).toBe("first");
    expect(inventoryParams?.messageProvider).toBe("telegram");
    expect(inventoryParams?.modelProvider).toBe("openai");
    expect(inventoryParams?.modelId).toBe("gpt-4.1");
  });

  it("reports configured MCP servers as not connected without starting them", async () => {
    runtimeMocks.resolveSessionMcpConfigSummary.mockReturnValueOnce({
      fingerprint: "mcp:1:test",
      serverNames: ["reproProbe"],
    });
    const { respond, invoke } = createInvokeParams({ sessionKey: "main:abc" });
    await invoke();

    const payload = firstRespondCall(respond)?.[1] as ToolsEffectivePayload | undefined;
    expect(payload?.groups?.map((group) => group.id)).toEqual(["core"]);
    expect(payload?.notices?.[0]?.id).toBe("mcp-not-yet-connected");
    expect(payload?.notices?.[0]?.message).toContain("reproProbe");
    expect(runtimeMocks.getOrCreateSessionMcpRuntime).not.toHaveBeenCalled();
    expect(runtimeMocks.materializeBundleMcpToolsForRun).not.toHaveBeenCalled();
  });

  it("projects MCP tools from an already-populated session runtime catalog", async () => {
    const mcpTool = makeMcpTool();
    const catalog: McpToolCatalog = { version: 1, generatedAt: 1, servers: {}, tools: [] };
    runtimeMocks.resolveSessionMcpConfigSummary.mockReturnValueOnce({
      fingerprint: "mcp:1:test",
      serverNames: ["reproProbe"],
    });
    runtimeMocks.peekSessionMcpRuntime.mockReturnValueOnce({
      configFingerprint: "mcp:1:test",
      peekCatalog: () => catalog,
    });
    runtimeMocks.buildBundleMcpToolsFromCatalog.mockReturnValueOnce([mcpTool]);

    const { respond, invoke } = createInvokeParams({ sessionKey: "main:abc" });
    await invoke();

    const payload = firstRespondCall(respond)?.[1] as ToolsEffectivePayload | undefined;
    expect(payload?.groups?.map((group) => group.id)).toEqual(["core", "mcp"]);
    expect(payload?.groups?.[1]).toEqual({
      id: "mcp",
      label: "MCP server tools",
      source: "mcp",
      tools: [
        {
          id: "reproProbe__probe_tool",
          label: "Probe Tool",
          description: "Probe from MCP",
          rawDescription: "Probe from MCP",
          source: "mcp",
          pluginId: "bundle-mcp",
        },
      ],
    });
    expect(runtimeMocks.buildBundleMcpToolsFromCatalog).toHaveBeenCalledWith({
      catalog,
      reservedToolNames: ["exec"],
    });
    expect(runtimeMocks.getOrCreateSessionMcpRuntime).not.toHaveBeenCalled();
    expect(runtimeMocks.materializeBundleMcpToolsForRun).not.toHaveBeenCalled();
  });

  it("does not project stale MCP catalogs after config changes", async () => {
    runtimeMocks.resolveSessionMcpConfigSummary.mockReturnValueOnce({
      fingerprint: "mcp:2:test",
      serverNames: ["reproProbe"],
    });
    runtimeMocks.peekSessionMcpRuntime.mockReturnValueOnce({
      configFingerprint: "mcp:1:test",
      peekCatalog: () => ({ version: 1, generatedAt: 1, servers: {}, tools: [] }),
    });

    const { respond, invoke } = createInvokeParams({ sessionKey: "main:abc" });
    await invoke();

    const payload = firstRespondCall(respond)?.[1] as ToolsEffectivePayload | undefined;
    expect(payload?.groups?.map((group) => group.id)).toEqual(["core"]);
    expect(payload?.notices?.[0]?.id).toBe("mcp-needs-refresh");
    expect(runtimeMocks.buildBundleMcpToolsFromCatalog).not.toHaveBeenCalled();
  });

  it("live-refreshes bundled MCP tools in a dedicated effective group", async () => {
    const dispose = vi.fn(async () => undefined);
    const mcpTool = makeMcpTool();
    const runtime = { sessionId: "session-1" };
    runtimeMocks.getOrCreateSessionMcpRuntime.mockResolvedValueOnce(runtime);
    runtimeMocks.materializeBundleMcpToolsForRun.mockResolvedValueOnce({
      tools: [mcpTool],
      dispose,
    });

    const { respond, invoke } = createInvokeParams(
      { sessionKey: "main:abc" },
      "tools.effective.refresh",
    );
    await invoke();

    const payload = firstRespondCall(respond)?.[1] as ToolsEffectivePayload | undefined;
    expect(payload?.groups?.map((group) => group.id)).toEqual(["core", "mcp"]);
    expect(runtimeMocks.getOrCreateSessionMcpRuntime).toHaveBeenCalledWith({
      sessionId: "session-1",
      sessionKey: "main:abc",
      workspaceDir: "/tmp/workspace-main",
      cfg: {},
    });
    expect(runtimeMocks.materializeBundleMcpToolsForRun).toHaveBeenCalledWith({
      runtime,
      reservedToolNames: ["exec"],
    });
    expect(runtimeMocks.applyFinalEffectiveToolPolicy).toHaveBeenCalledWith(
      expect.objectContaining({
        bundledTools: [mcpTool],
        config: {},
        sessionKey: "main:abc",
        agentId: "main",
        modelProvider: "openai",
        modelId: "gpt-4.1",
        messageProvider: "telegram",
        agentAccountId: "acct-1",
        groupId: "group-4",
        groupChannel: "#ops",
        groupSpace: "workspace-5",
        spawnedBy: "agent:main:telegram:group:parent-group",
      }),
    );
    expect(dispose).toHaveBeenCalledTimes(1);
  });

  it("does not report refreshed MCP tools filtered out by final policy", async () => {
    const dispose = vi.fn(async () => undefined);
    const mcpTool = makeMcpTool();
    runtimeMocks.materializeBundleMcpToolsForRun.mockResolvedValueOnce({
      tools: [mcpTool],
      dispose,
    });
    runtimeMocks.applyFinalEffectiveToolPolicy.mockReturnValueOnce([]);

    const { respond, invoke } = createInvokeParams(
      { sessionKey: "main:abc" },
      "tools.effective.refresh",
    );
    await invoke();

    const payload = firstRespondCall(respond)?.[1] as ToolsEffectivePayload | undefined;
    expect(payload?.groups?.map((group) => group.id)).toEqual(["core"]);
    expect(dispose).toHaveBeenCalledTimes(1);
  });

  it("returns a warning notice when refresh materialization fails", async () => {
    runtimeMocks.resolveSessionMcpConfigSummary.mockReturnValueOnce({
      fingerprint: "mcp:1:test",
      serverNames: ["reproProbe"],
    });
    runtimeMocks.getOrCreateSessionMcpRuntime.mockRejectedValueOnce(new Error("boom"));

    const { respond, invoke } = createInvokeParams(
      { sessionKey: "main:abc" },
      "tools.effective.refresh",
    );
    await invoke();

    const payload = firstRespondCall(respond)?.[1] as ToolsEffectivePayload | undefined;
    expect(payload?.groups?.map((group) => group.id)).toEqual(["core"]);
    expect(payload?.notices?.[0]?.id).toBe("mcp-refresh-failed");
  });

  it("falls back to origin.threadId when delivery context omits thread metadata", async () => {
    runtimeMocks.loadSessionEntry.mockReturnValueOnce({
      cfg: {},
      canonicalKey: "main:abc",
      entry: {
        sessionId: "session-origin-thread",
        updatedAt: 1,
        lastChannel: "telegram",
        lastAccountId: "acct-1",
        lastTo: "channel-1",
        origin: {
          provider: "telegram",
          accountId: "acct-1",
          threadId: 42,
        },
        groupId: "group-4",
        groupChannel: "#ops",
        space: "workspace-5",
        chatType: "group",
        modelProvider: "openai",
        model: "gpt-4.1",
      },
    } as never);
    runtimeMocks.deliveryContextFromSession.mockReturnValueOnce({
      channel: "telegram",
      to: "channel-1",
      accountId: "acct-1",
      threadId: "42",
    });

    const { respond, invoke } = createInvokeParams({ sessionKey: "main:abc" });
    await invoke();

    expect(resolveEffectiveToolInventoryArg()?.currentThreadTs).toBe("42");
    expect(firstRespondCall(respond)?.[0]).toBe(true);
  });

  it("rejects agent ids that do not match the session agent", async () => {
    const { respond, invoke } = createInvokeParams({
      sessionKey: "main:abc",
      agentId: "other",
    });
    runtimeMocks.loadSessionEntry.mockReturnValueOnce({
      cfg: {},
      canonicalKey: "main:abc",
      entry: {
        sessionId: "session-1",
        updatedAt: 1,
      },
    } as never);
    await invoke();
    const call = firstRespondCall(respond);
    expect(call?.[0]).toBe(false);
    expect(call?.[2]?.code).toBe(ErrorCodes.INVALID_REQUEST);
    expect(call?.[2]?.message).toContain('unknown agent id "other"');
  });
});
