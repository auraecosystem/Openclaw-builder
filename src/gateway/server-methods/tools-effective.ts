import {
  buildEffectiveToolInventoryEntries,
  buildEffectiveToolInventoryGroups,
} from "../../agents/tools-effective-inventory.js";
import type {
  EffectiveToolInventoryNotice,
  EffectiveToolInventoryResult,
} from "../../agents/tools-effective-inventory.types.js";
import type { OpenClawConfig } from "../../config/types.openclaw.js";
import { logDebug, logWarn } from "../../logger.js";
import { stringifyRouteThreadId } from "../../plugin-sdk/channel-route.js";
import { normalizeOptionalString } from "../../shared/string-coerce.js";
import {
  ErrorCodes,
  errorShape,
  formatValidationErrors,
  validateToolsEffectiveParams,
} from "../protocol/index.js";
import {
  applyFinalEffectiveToolPolicy,
  buildBundleMcpToolsFromCatalog,
  deliveryContextFromSession,
  getOrCreateSessionMcpRuntime,
  listAgentIds,
  loadSessionEntry,
  materializeBundleMcpToolsForRun,
  peekSessionMcpRuntime,
  resolveAgentWorkspaceDir,
  resolveEffectiveToolInventory,
  resolveReplyToMode,
  resolveSessionAgentId,
  resolveSessionMcpConfigSummary,
  resolveSessionModelRef,
} from "./tools-effective.runtime.js";
import type { GatewayRequestHandlers, RespondFn } from "./types.js";

const TOOLS_EFFECTIVE_SLOW_LOG_MS = 250;

type TrustedToolsEffectiveContext = {
  cfg: OpenClawConfig;
  agentId: string;
  sessionKey: string;
  sessionId: string;
  workspaceDir: string;
  mcpConfigFingerprint: string;
  mcpServerNames: string[];
  modelProvider?: string;
  modelId?: string;
  messageProvider?: string;
  accountId?: string;
  currentChannelId?: string;
  currentThreadTs?: string;
  groupId?: string | null;
  groupChannel?: string | null;
  groupSpace?: string | null;
  replyToMode?: "off" | "first" | "all" | "batched";
  spawnedBy?: string | null;
};

function resolveRequestedAgentIdOrRespondError(params: {
  rawAgentId: unknown;
  cfg: OpenClawConfig;
  respond: RespondFn;
}) {
  const knownAgents = listAgentIds(params.cfg);
  const requestedAgentId = normalizeOptionalString(params.rawAgentId) ?? "";
  if (!requestedAgentId) {
    return undefined;
  }
  if (!knownAgents.includes(requestedAgentId)) {
    params.respond(
      false,
      undefined,
      errorShape(ErrorCodes.INVALID_REQUEST, `unknown agent id "${requestedAgentId}"`),
    );
    return null;
  }
  return requestedAgentId;
}

function appendMcpInventoryGroups(params: {
  base: EffectiveToolInventoryResult;
  mcpTools: Parameters<typeof buildEffectiveToolInventoryEntries>[0];
}): EffectiveToolInventoryResult {
  if (params.mcpTools.length === 0) {
    return params.base;
  }
  const mcpEntries = buildEffectiveToolInventoryEntries(params.mcpTools).filter(
    (entry) => entry.source === "mcp",
  );
  if (mcpEntries.length === 0) {
    return params.base;
  }
  const mcpGroups = buildEffectiveToolInventoryGroups(mcpEntries);
  return {
    ...params.base,
    groups: [...params.base.groups, ...mcpGroups],
  };
}

function appendToolInventoryNotice(
  base: EffectiveToolInventoryResult,
  notice: EffectiveToolInventoryNotice,
): EffectiveToolInventoryResult {
  return {
    ...base,
    notices: [...(base.notices ?? []), notice],
  };
}

function formatMcpServerNames(names: readonly string[]): string {
  if (names.length === 0) {
    return "configured MCP servers";
  }
  const visible = names
    .slice(0, 3)
    .map((name) => `"${name}"`)
    .join(", ");
  return names.length > 3 ? `${visible}, and ${names.length - 3} more MCP servers` : visible;
}

function mcpDiscoveryNotice(
  context: TrustedToolsEffectiveContext,
  reason: "not-connected" | "not-listed" | "stale-config" | "refresh-failed",
): EffectiveToolInventoryNotice | undefined {
  if (context.mcpServerNames.length === 0) {
    return undefined;
  }
  const servers = formatMcpServerNames(context.mcpServerNames);
  switch (reason) {
    case "stale-config":
      return {
        id: "mcp-needs-refresh",
        severity: "info",
        message: `MCP servers ${servers} changed since the current runtime catalog was discovered. Refresh available tools to list MCP tools again.`,
      };
    case "refresh-failed":
      return {
        id: "mcp-refresh-failed",
        severity: "warning",
        message: `MCP servers ${servers} could not be listed. Check the gateway logs, then refresh available tools again.`,
      };
    case "not-listed":
      return {
        id: "mcp-not-yet-listed",
        severity: "info",
        message: `MCP servers ${servers} are connected but have not finished listing tools yet. Refresh available tools or send a message to discover them.`,
      };
    case "not-connected":
      return {
        id: "mcp-not-yet-connected",
        severity: "info",
        message: `MCP servers ${servers} are configured but not connected for this session yet. Refresh available tools or send a message to discover them.`,
      };
    default:
      // Exhaustiveness guard for oxlint's consistent-return rule.
      return undefined;
  }
}

function maybeAppendMcpNotice(
  base: EffectiveToolInventoryResult,
  context: TrustedToolsEffectiveContext,
  reason: "not-connected" | "not-listed" | "stale-config" | "refresh-failed",
): EffectiveToolInventoryResult {
  const notice = mcpDiscoveryNotice(context, reason);
  return notice ? appendToolInventoryNotice(base, notice) : base;
}

function resolveBaseToolsEffectiveInventory(
  context: TrustedToolsEffectiveContext,
): EffectiveToolInventoryResult {
  return resolveEffectiveToolInventory({
    cfg: context.cfg,
    agentId: context.agentId,
    sessionKey: context.sessionKey,
    workspaceDir: context.workspaceDir,
    messageProvider: context.messageProvider,
    modelProvider: context.modelProvider,
    modelId: context.modelId,
    currentChannelId: context.currentChannelId,
    currentThreadTs: context.currentThreadTs,
    accountId: context.accountId,
    groupId: context.groupId,
    groupChannel: context.groupChannel,
    groupSpace: context.groupSpace,
    replyToMode: context.replyToMode,
  });
}

function filterMcpTools(params: {
  context: TrustedToolsEffectiveContext;
  mcpTools: Parameters<typeof applyFinalEffectiveToolPolicy>[0]["bundledTools"];
}) {
  return applyFinalEffectiveToolPolicy({
    bundledTools: params.mcpTools,
    config: params.context.cfg,
    sessionKey: params.context.sessionKey,
    agentId: params.context.agentId,
    modelProvider: params.context.modelProvider,
    modelId: params.context.modelId,
    messageProvider: params.context.messageProvider,
    agentAccountId: params.context.accountId,
    groupId: params.context.groupId,
    groupChannel: params.context.groupChannel,
    groupSpace: params.context.groupSpace,
    spawnedBy: params.context.spawnedBy,
    warn: logWarn,
  });
}

function resolveReadOnlyToolsEffectiveInventory(
  context: TrustedToolsEffectiveContext,
): EffectiveToolInventoryResult {
  const base = resolveBaseToolsEffectiveInventory(context);
  if (context.mcpServerNames.length === 0) {
    return base;
  }
  const runtime = peekSessionMcpRuntime({
    sessionId: context.sessionId,
    sessionKey: context.sessionKey,
  });
  if (!runtime) {
    return maybeAppendMcpNotice(base, context, "not-connected");
  }
  if (runtime.configFingerprint !== context.mcpConfigFingerprint) {
    return maybeAppendMcpNotice(base, context, "stale-config");
  }
  const catalog = runtime.peekCatalog();
  if (!catalog) {
    return maybeAppendMcpNotice(base, context, "not-listed");
  }
  const projectedMcpTools = buildBundleMcpToolsFromCatalog({
    catalog,
    reservedToolNames: base.groups.flatMap((group) => group.tools.map((tool) => tool.id)),
  });
  const filteredMcpTools = filterMcpTools({ context, mcpTools: projectedMcpTools });
  return appendMcpInventoryGroups({ base, mcpTools: filteredMcpTools });
}

async function resolveLiveToolsEffectiveInventory(
  context: TrustedToolsEffectiveContext,
): Promise<EffectiveToolInventoryResult> {
  const startedAt = Date.now();
  const base = resolveBaseToolsEffectiveInventory(context);
  let materialized: Awaited<ReturnType<typeof materializeBundleMcpToolsForRun>> | undefined;
  try {
    const runtime = await getOrCreateSessionMcpRuntime({
      sessionId: context.sessionId,
      sessionKey: context.sessionKey,
      workspaceDir: context.workspaceDir,
      cfg: context.cfg,
    });
    materialized = await materializeBundleMcpToolsForRun({
      runtime,
      reservedToolNames: base.groups.flatMap((group) => group.tools.map((tool) => tool.id)),
    });
    const filteredMcpTools = filterMcpTools({ context, mcpTools: materialized.tools });
    const value = appendMcpInventoryGroups({ base, mcpTools: filteredMcpTools });
    const durationMs = Date.now() - startedAt;
    if (durationMs >= TOOLS_EFFECTIVE_SLOW_LOG_MS) {
      logDebug(
        `tools-effective.refresh: durationMs=${durationMs} agent=${context.agentId} session=${context.sessionKey} tools=${value.groups.reduce((sum, group) => sum + group.tools.length, 0)}`,
      );
    }
    return value;
  } catch (err) {
    logWarn(`tools-effective.refresh: MCP inventory materialization failed: ${String(err)}`);
    return maybeAppendMcpNotice(base, context, "refresh-failed");
  } finally {
    try {
      await materialized?.dispose();
    } catch {
      /* best-effort lease release */
    }
  }
}

function resolveTrustedToolsEffectiveContext(params: {
  sessionKey: string;
  requestedAgentId?: string;
  respond: RespondFn;
}) {
  const loaded = loadSessionEntry(params.sessionKey);
  if (!loaded.entry) {
    params.respond(
      false,
      undefined,
      errorShape(ErrorCodes.INVALID_REQUEST, `unknown session key "${params.sessionKey}"`),
    );
    return null;
  }

  const sessionAgentId = resolveSessionAgentId({
    sessionKey: loaded.canonicalKey ?? params.sessionKey,
    config: loaded.cfg,
  });
  if (params.requestedAgentId && params.requestedAgentId !== sessionAgentId) {
    params.respond(
      false,
      undefined,
      errorShape(
        ErrorCodes.INVALID_REQUEST,
        `agent id "${params.requestedAgentId}" does not match session agent "${sessionAgentId}"`,
      ),
    );
    return null;
  }

  const delivery = deliveryContextFromSession(loaded.entry);
  const resolvedModel = resolveSessionModelRef(loaded.cfg, loaded.entry, sessionAgentId);
  const workspaceDir =
    normalizeOptionalString(loaded.entry.spawnedWorkspaceDir) ??
    resolveAgentWorkspaceDir(loaded.cfg, sessionAgentId);
  const mcpConfig = resolveSessionMcpConfigSummary({ workspaceDir, cfg: loaded.cfg });
  return {
    cfg: loaded.cfg,
    agentId: sessionAgentId,
    sessionKey: params.sessionKey,
    sessionId: loaded.entry.sessionId,
    workspaceDir,
    mcpConfigFingerprint: mcpConfig.fingerprint,
    mcpServerNames: mcpConfig.serverNames,
    modelProvider: resolvedModel.provider,
    modelId: resolvedModel.model,
    messageProvider:
      delivery?.channel ??
      loaded.entry.lastChannel ??
      loaded.entry.channel ??
      loaded.entry.origin?.provider,
    accountId: delivery?.accountId ?? loaded.entry.lastAccountId ?? loaded.entry.origin?.accountId,
    currentChannelId: delivery?.to,
    currentThreadTs:
      delivery?.threadId != null
        ? stringifyRouteThreadId(delivery.threadId)
        : loaded.entry.lastThreadId != null
          ? stringifyRouteThreadId(loaded.entry.lastThreadId)
          : loaded.entry.origin?.threadId != null
            ? stringifyRouteThreadId(loaded.entry.origin.threadId)
            : undefined,
    groupId: loaded.entry.groupId,
    groupChannel: loaded.entry.groupChannel,
    groupSpace: loaded.entry.space,
    spawnedBy: normalizeOptionalString(loaded.entry.spawnedBy),
    replyToMode: resolveReplyToMode(
      loaded.cfg,
      delivery?.channel ??
        loaded.entry.lastChannel ??
        loaded.entry.channel ??
        loaded.entry.origin?.provider,
      delivery?.accountId ?? loaded.entry.lastAccountId ?? loaded.entry.origin?.accountId,
      loaded.entry.chatType ?? loaded.entry.origin?.chatType,
    ),
  };
}

async function handleToolsEffectiveRequest(params: {
  method: "tools.effective" | "tools.effective.refresh";
  mode: "read" | "refresh";
  rawParams: unknown;
  respond: RespondFn;
  context: Parameters<GatewayRequestHandlers[string]>[0]["context"];
}) {
  if (!validateToolsEffectiveParams(params.rawParams)) {
    params.respond(
      false,
      undefined,
      errorShape(
        ErrorCodes.INVALID_REQUEST,
        `invalid ${params.method} params: ${formatValidationErrors(validateToolsEffectiveParams.errors)}`,
      ),
    );
    return;
  }
  const cfg = params.context.getRuntimeConfig();
  const requestedAgentId = resolveRequestedAgentIdOrRespondError({
    rawAgentId: params.rawParams.agentId,
    cfg,
    respond: params.respond,
  });
  if (requestedAgentId === null) {
    return;
  }
  const trustedContext = resolveTrustedToolsEffectiveContext({
    sessionKey: params.rawParams.sessionKey,
    requestedAgentId,
    respond: params.respond,
  });
  if (!trustedContext) {
    return;
  }
  try {
    params.respond(
      true,
      params.mode === "refresh"
        ? await resolveLiveToolsEffectiveInventory(trustedContext)
        : resolveReadOnlyToolsEffectiveInventory(trustedContext),
      undefined,
    );
  } catch (err) {
    params.respond(
      false,
      undefined,
      errorShape(ErrorCodes.UNAVAILABLE, `${params.method} failed: ${String(err)}`),
    );
  }
}

export const toolsEffectiveHandlers: GatewayRequestHandlers = {
  "tools.effective": async ({ params, respond, context }) => {
    await handleToolsEffectiveRequest({
      method: "tools.effective",
      mode: "read",
      rawParams: params,
      respond,
      context,
    });
  },
  "tools.effective.refresh": async ({ params, respond, context }) => {
    await handleToolsEffectiveRequest({
      method: "tools.effective.refresh",
      mode: "refresh",
      rawParams: params,
      respond,
      context,
    });
  },
};

export const testing = {
  resetToolsEffectiveCacheForTest() {
    /* no request-level cache: read path peeks current runtime state */
  },
  setToolsEffectiveNowForTest(_now: () => number) {
    /* retained for older focused tests */
  },
  resetToolsEffectiveNowForTest() {
    /* retained for older focused tests */
  },
} as const;
export { testing as __testing };
