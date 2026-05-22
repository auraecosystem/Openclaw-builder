import {
  normalizeChatModelOverrideValue,
  resolvePreferredServerChatModelValue,
} from "../chat-model-ref.ts";
import type { GatewayBrowserClient } from "../gateway.ts";
import { t } from "../../i18n/index.ts";
import { deriveAgentId, normalizeAgentId, resolveAgentIdFromSessionKey } from "../session-key.ts";
import type {
  AgentsCreateResult,
  AgentsListResult,
  ChatModelOverride,
  ModelCatalogEntry,
  SessionsListResult,
  ToolsCatalogResult,
  ToolsEffectiveResult,
} from "../types.ts";
import { saveConfig } from "./config.ts";
import type { ConfigState } from "./config.ts";
import {
  formatMissingOperatorReadScopeMessage,
  isMissingOperatorReadScopeError,
} from "./scope-errors.ts";

export type AgentCreateDraft = {
  name: string;
  workspace: string;
  model: string;
  emoji: string;
  avatar: string;
};

export type AgentCreateValidationError = {
  key: string;
  vars?: Record<string, string>;
};

export type AgentsState = {
  client: GatewayBrowserClient | null;
  connected: boolean;
  agentsLoading: boolean;
  agentsLoadRequestId?: number;
  agentsError: string | null;
  agentsList: AgentsListResult | null;
  agentsSelectedId: string | null;
  toolsCatalogLoading: boolean;
  toolsCatalogLoadingAgentId?: string | null;
  toolsCatalogError: string | null;
  toolsCatalogResult: ToolsCatalogResult | null;
  toolsEffectiveLoading: boolean;
  toolsEffectiveLoadingKey?: string | null;
  toolsEffectiveResultKey?: string | null;
  toolsEffectiveError: string | null;
  toolsEffectiveResult: ToolsEffectiveResult | null;
  sessionKey?: string;
  sessionsResult?: SessionsListResult | null;
  chatModelOverrides?: Record<string, ChatModelOverride | null>;
  chatModelCatalog?: ModelCatalogEntry[];
  agentsPanel?: "overview" | "files" | "tools" | "skills" | "channels" | "cron";
};

export type AgentsConfigSaveState = AgentsState & ConfigState;

const DEFAULT_CREATE_AGENT_WORKSPACE = "~/.openclaw/workspace-agent";

function splitWorkspacePath(value: string): { parent: string; leaf: string; separator: string } {
  const trimmed = value.trim().replace(/[\\/]+$/, "");
  const separator = trimmed.includes("\\") && !trimmed.includes("/") ? "\\" : "/";
  const slash = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  if (slash < 0) {
    return { parent: "", leaf: trimmed, separator };
  }
  return {
    parent: trimmed.slice(0, slash),
    leaf: trimmed.slice(slash + 1),
    separator: trimmed[slash] ?? separator,
  };
}

function joinWorkspacePath(parent: string, leaf: string, separator: string): string {
  return parent ? `${parent}${separator}${leaf}` : leaf;
}

export function buildDefaultCreateAgentWorkspace(
  agentsList: AgentsListResult | null | undefined,
  agentIdInput: string,
): string {
  const agentId = normalizeAgentId(agentIdInput);
  const suffix = agentId && agentId !== "main" ? agentId : "agent";
  const defaultAgentId = normalizeAgentId(agentsList?.defaultId);
  const defaultWorkspace =
    agentsList?.agents.find((entry) => normalizeAgentId(entry.id) === defaultAgentId)?.workspace ??
    agentsList?.agents[0]?.workspace;
  const base = defaultWorkspace?.trim();
  if (!base) {
    return suffix === "agent" ? DEFAULT_CREATE_AGENT_WORKSPACE : `~/.openclaw/workspace-${suffix}`;
  }
  const { parent, leaf, separator } = splitWorkspacePath(base);
  if (!leaf) {
    return suffix === "agent" ? DEFAULT_CREATE_AGENT_WORKSPACE : `~/.openclaw/workspace-${suffix}`;
  }
  const nextLeaf = /^workspace(?:-[a-z0-9_-]+)?$/i.test(leaf)
    ? `workspace-${suffix}`
    : `${leaf}-${suffix}`;
  return joinWorkspacePath(parent, nextLeaf, separator);
}

export function createDefaultAgentCreateDraft(
  agentsList?: AgentsListResult | null,
): AgentCreateDraft {
  return {
    name: "",
    workspace: buildDefaultCreateAgentWorkspace(agentsList, ""),
    model: "",
    emoji: "",
    avatar: "",
  };
}

export function validateAgentCreateDraft(
  draft: AgentCreateDraft,
  agentsList?: AgentsListResult | null,
): AgentCreateValidationError | null {
  const name = draft.name.trim();
  if (!name) {
    return { key: "agents.create.errors.nameRequired" };
  }
  const agentId = deriveAgentId(name);
  if (!agentId) {
    return { key: "agents.create.errors.invalidName" };
  }
  if (agentId === "main") {
    return { key: "agents.create.errors.mainReserved" };
  }
  if (agentsList?.agents.some((entry) => normalizeAgentId(entry.id) === agentId)) {
    return { key: "agents.create.errors.duplicate", vars: { id: agentId } };
  }
  if (!draft.workspace.trim()) {
    return { key: "agents.create.errors.workspaceRequired" };
  }
  return null;
}

export async function createAgentFromDraft(
  state: AgentsState,
  draft: AgentCreateDraft,
): Promise<AgentsCreateResult> {
  if (!state.client || !state.connected) {
    throw new Error("Gateway is not connected.");
  }
  const validationError = validateAgentCreateDraft(draft, state.agentsList);
  if (validationError) {
    throw new Error(t(validationError.key, validationError.vars ?? {}));
  }
  const payload: {
    name: string;
    workspace: string;
    model?: string;
    emoji?: string;
    avatar?: string;
  } = {
    name: draft.name.trim(),
    workspace: draft.workspace.trim(),
  };
  const model = draft.model.trim();
  if (model) {
    payload.model = model;
  }
  const emoji = draft.emoji.trim();
  if (emoji) {
    payload.emoji = emoji;
  }
  const avatar = draft.avatar.trim();
  if (avatar) {
    payload.avatar = avatar;
  }
  const created = await state.client.request<AgentsCreateResult>("agents.create", payload);
  await loadAgents(state, { force: true });
  if (!state.agentsList?.agents.some((entry) => normalizeAgentId(entry.id) === created.agentId)) {
    throw new Error(`Created agent "${created.agentId}" was not returned by agents.list.`);
  }
  state.agentsSelectedId = created.agentId;
  return created;
}

function hasSelectedAgentMismatch(state: AgentsState, agentId: string): boolean {
  return Boolean(state.agentsSelectedId && state.agentsSelectedId !== agentId);
}

function resolveToolsErrorMessage(
  err: unknown,
  target: "tools catalog" | "effective tools",
): string {
  return isMissingOperatorReadScopeError(err)
    ? formatMissingOperatorReadScopeMessage(target)
    : String(err);
}

export async function loadAgents(state: AgentsState, options?: { force?: boolean }) {
  if (!state.client || !state.connected || (state.agentsLoading && !options?.force)) {
    return;
  }
  const requestId = (state.agentsLoadRequestId ?? 0) + 1;
  state.agentsLoadRequestId = requestId;
  const shouldIgnoreResponse = () => state.agentsLoadRequestId !== requestId;
  state.agentsLoading = true;
  state.agentsError = null;
  try {
    const res = await state.client.request<AgentsListResult>("agents.list", {});
    if (shouldIgnoreResponse()) {
      return;
    }
    if (res) {
      state.agentsList = res;
      const selected = state.agentsSelectedId;
      if (!selected || !res.agents.some((entry) => entry.id === selected)) {
        state.agentsSelectedId = res.defaultId ?? res.agents[0]?.id ?? null;
      }
    }
  } catch (err) {
    if (shouldIgnoreResponse()) {
      return;
    }
    if (isMissingOperatorReadScopeError(err)) {
      state.agentsList = null;
      state.agentsError = formatMissingOperatorReadScopeMessage("agent list");
    } else {
      state.agentsError = String(err);
    }
  } finally {
    if (!shouldIgnoreResponse()) {
      state.agentsLoading = false;
    }
  }
}

export async function loadToolsCatalog(state: AgentsState, agentId: string) {
  const resolvedAgentId = agentId.trim();
  if (
    !state.client ||
    !state.connected ||
    !resolvedAgentId ||
    (state.toolsCatalogLoading && state.toolsCatalogLoadingAgentId === resolvedAgentId)
  ) {
    return;
  }
  const shouldIgnoreResponse = () =>
    state.toolsCatalogLoadingAgentId !== resolvedAgentId ||
    hasSelectedAgentMismatch(state, resolvedAgentId);
  state.toolsCatalogLoading = true;
  state.toolsCatalogLoadingAgentId = resolvedAgentId;
  state.toolsCatalogError = null;
  state.toolsCatalogResult = null;
  try {
    const res = await state.client.request<ToolsCatalogResult>("tools.catalog", {
      agentId: resolvedAgentId,
      includePlugins: true,
    });
    if (shouldIgnoreResponse()) {
      return;
    }
    state.toolsCatalogResult = res;
  } catch (err) {
    if (shouldIgnoreResponse()) {
      return;
    }
    state.toolsCatalogError = resolveToolsErrorMessage(err, "tools catalog");
  } finally {
    if (state.toolsCatalogLoadingAgentId === resolvedAgentId) {
      state.toolsCatalogLoadingAgentId = null;
      state.toolsCatalogLoading = false;
    }
  }
}

export async function loadToolsEffective(
  state: AgentsState,
  params: { agentId: string; sessionKey: string },
) {
  const resolvedAgentId = params.agentId.trim();
  const resolvedSessionKey = params.sessionKey.trim();
  const requestKey = buildToolsEffectiveRequestKey(state, {
    agentId: resolvedAgentId,
    sessionKey: resolvedSessionKey,
  });
  if (
    !state.client ||
    !state.connected ||
    !resolvedAgentId ||
    !resolvedSessionKey ||
    (state.toolsEffectiveLoading && state.toolsEffectiveLoadingKey === requestKey)
  ) {
    return;
  }
  const shouldIgnoreResponse = () =>
    state.toolsEffectiveLoadingKey !== requestKey ||
    hasSelectedAgentMismatch(state, resolvedAgentId);
  state.toolsEffectiveLoading = true;
  state.toolsEffectiveLoadingKey = requestKey;
  state.toolsEffectiveResultKey = null;
  state.toolsEffectiveError = null;
  state.toolsEffectiveResult = null;
  try {
    const res = await state.client.request<ToolsEffectiveResult>("tools.effective", {
      agentId: resolvedAgentId,
      sessionKey: resolvedSessionKey,
    });
    if (shouldIgnoreResponse()) {
      return;
    }
    state.toolsEffectiveResultKey = requestKey;
    state.toolsEffectiveResult = res;
  } catch (err) {
    if (shouldIgnoreResponse()) {
      return;
    }
    state.toolsEffectiveError = resolveToolsErrorMessage(err, "effective tools");
  } finally {
    if (state.toolsEffectiveLoadingKey === requestKey) {
      state.toolsEffectiveLoadingKey = null;
      state.toolsEffectiveLoading = false;
    }
  }
}

export function resetToolsEffectiveState(state: AgentsState) {
  state.toolsEffectiveResult = null;
  state.toolsEffectiveResultKey = null;
  state.toolsEffectiveError = null;
  state.toolsEffectiveLoading = false;
  state.toolsEffectiveLoadingKey = null;
}

export function buildToolsEffectiveRequestKey(
  state: Pick<AgentsState, "sessionsResult" | "chatModelOverrides" | "chatModelCatalog">,
  params: { agentId: string; sessionKey: string },
): string {
  const resolvedAgentId = params.agentId.trim();
  const resolvedSessionKey = params.sessionKey.trim();
  const modelKey = resolveEffectiveToolsModelKey(state, resolvedSessionKey);
  return `${resolvedAgentId}:${resolvedSessionKey}:model=${modelKey || "(default)"}`;
}

export function refreshVisibleToolsEffectiveForCurrentSession(
  state: AgentsState,
): Promise<void> | undefined {
  const resolvedSessionKey = state.sessionKey?.trim();
  if (!resolvedSessionKey || state.agentsPanel !== "tools" || !state.agentsSelectedId) {
    return undefined;
  }
  const sessionAgentId = resolveAgentIdFromSessionKey(resolvedSessionKey);
  if (!sessionAgentId || state.agentsSelectedId !== sessionAgentId) {
    return undefined;
  }
  return loadToolsEffective(state, {
    agentId: sessionAgentId,
    sessionKey: resolvedSessionKey,
  });
}

function resolveEffectiveToolsModelKey(
  state: Pick<AgentsState, "sessionsResult" | "chatModelOverrides" | "chatModelCatalog">,
  sessionKey: string,
): string {
  const resolvedSessionKey = sessionKey.trim();
  if (!resolvedSessionKey) {
    return "";
  }
  const catalog = state.chatModelCatalog ?? [];
  const cachedOverride = state.chatModelOverrides?.[resolvedSessionKey];
  const defaults = state.sessionsResult?.defaults;
  const defaultModel = resolvePreferredServerChatModelValue(
    defaults?.model,
    defaults?.modelProvider,
    catalog,
  );
  if (cachedOverride === null) {
    return defaultModel;
  }
  if (cachedOverride) {
    return normalizeChatModelOverrideValue(cachedOverride, catalog);
  }
  const activeRow = state.sessionsResult?.sessions?.find((row) => row.key === resolvedSessionKey);
  if (activeRow?.model) {
    return resolvePreferredServerChatModelValue(activeRow.model, activeRow.modelProvider, catalog);
  }
  return defaultModel;
}

export async function saveAgentsConfig(state: AgentsConfigSaveState) {
  const selectedBefore = state.agentsSelectedId;
  await saveConfig(state);
  await loadAgents(state);
  if (selectedBefore && state.agentsList?.agents.some((entry) => entry.id === selectedBefore)) {
    state.agentsSelectedId = selectedBefore;
  }
}
