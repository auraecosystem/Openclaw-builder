import crypto from "node:crypto";
import type { InboundEventKind } from "../channels/inbound-event/kind.js";

type McpLoopbackRuntime = {
  port: number;
  ownerToken: string;
  nonOwnerToken: string;
};

export type McpLoopbackTokenScope = {
  sessionKey?: string;
  messageProvider?: string;
  accountId?: string;
  inboundEventKind?: InboundEventKind;
  senderIsOwner: boolean;
};

let activeRuntime: McpLoopbackRuntime | undefined;
const scopedTokenContexts = new Map<string, McpLoopbackTokenScope>();

function normalizeScopeString(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

export function getActiveMcpLoopbackRuntime(): McpLoopbackRuntime | undefined {
  return activeRuntime ? { ...activeRuntime } : undefined;
}

export function setActiveMcpLoopbackRuntime(runtime: McpLoopbackRuntime): void {
  activeRuntime = { ...runtime };
  scopedTokenContexts.clear();
}

export function resolveMcpLoopbackBearerToken(
  runtime: McpLoopbackRuntime,
  senderIsOwner: boolean,
): string {
  return senderIsOwner ? runtime.ownerToken : runtime.nonOwnerToken;
}

export function issueMcpLoopbackScopedBearerToken(
  runtime: McpLoopbackRuntime,
  scope: McpLoopbackTokenScope,
): string {
  if (
    !activeRuntime ||
    activeRuntime.ownerToken !== runtime.ownerToken ||
    activeRuntime.nonOwnerToken !== runtime.nonOwnerToken
  ) {
    throw new Error("mcp loopback runtime is not active");
  }
  const token = crypto.randomBytes(32).toString("hex");
  scopedTokenContexts.set(token, {
    sessionKey: normalizeScopeString(scope.sessionKey),
    messageProvider: normalizeScopeString(scope.messageProvider),
    accountId: normalizeScopeString(scope.accountId),
    inboundEventKind: scope.inboundEventKind,
    senderIsOwner: scope.senderIsOwner,
  });
  return token;
}

export function revokeMcpLoopbackScopedBearerToken(token: string | undefined): void {
  if (token) {
    scopedTokenContexts.delete(token);
  }
}

export function resolveMcpLoopbackScopedBearerTokenContext(
  authHeader: string,
): McpLoopbackTokenScope | undefined {
  const token = authHeader.startsWith("Bearer ") ? authHeader.slice("Bearer ".length).trim() : "";
  const context = token ? scopedTokenContexts.get(token) : undefined;
  return context ? { ...context } : undefined;
}

export function clearActiveMcpLoopbackRuntimeByOwnerToken(ownerToken: string): void {
  if (activeRuntime?.ownerToken === ownerToken) {
    activeRuntime = undefined;
    scopedTokenContexts.clear();
  }
}

export function createMcpLoopbackServerConfig(port: number) {
  return {
    mcpServers: {
      openclaw: {
        type: "http",
        url: `http://127.0.0.1:${port}/mcp`,
        headers: {
          Authorization: "Bearer ${OPENCLAW_MCP_TOKEN}",
        },
      },
    },
  };
}
