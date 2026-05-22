import { normalizeFastMode } from "../auto-reply/thinking.shared.js";
import type { SessionEntry } from "../config/sessions.js";
import type { OpenClawConfig } from "../config/types.openclaw.js";
import type { FastMode } from "../shared/string-coerce.js";
import { resolveAgentConfig } from "./agent-scope.js";
import { modelKey } from "./model-ref-shared.js";

export type { FastMode } from "../shared/string-coerce.js";

export const DEFAULT_FAST_MODE_AUTO_SECONDS = 60;

type FastModeState = {
  mode: FastMode;
  enabled: boolean;
  source: "session" | "agent" | "config" | "default";
  fastSeconds: number;
};

function resolveConfiguredFastModeRaw(params: {
  cfg: OpenClawConfig | undefined;
  provider: string;
  model: string;
}): unknown {
  const modelParams = resolveConfiguredModelParams(params);
  return modelParams?.fastMode ?? modelParams?.fast_mode;
}

function resolveConfiguredModelParams(params: {
  cfg: OpenClawConfig | undefined;
  provider: string;
  model: string;
}): Record<string, unknown> | undefined {
  const modelConfig =
    params.cfg?.agents?.defaults?.models?.[modelKey(params.provider, params.model)];
  return modelConfig?.params;
}

function normalizeFastSeconds(raw: unknown): number | undefined {
  const value =
    typeof raw === "number"
      ? raw
      : typeof raw === "string" && raw.trim()
        ? Number(raw.trim())
        : undefined;
  if (value === undefined || !Number.isFinite(value) || value <= 0) {
    return undefined;
  }
  return Math.ceil(value);
}

function resolveConfiguredFastSeconds(params: {
  cfg: OpenClawConfig | undefined;
  provider: string;
  model: string;
}): number {
  const modelParams = resolveConfiguredModelParams(params);
  return normalizeFastSeconds(modelParams?.fast_seconds) ?? DEFAULT_FAST_MODE_AUTO_SECONDS;
}

export function resolveFastModeAutoSeconds(params: {
  cfg: OpenClawConfig | undefined;
  provider: string;
  model: string;
}): number {
  return resolveConfiguredFastSeconds(params);
}

export function resolveFastModeForElapsed(params: {
  mode?: FastMode;
  fastSeconds?: number;
  startedAtMs: number;
  nowMs?: number;
}): {
  mode: FastMode | undefined;
  enabled: boolean;
  elapsedSeconds: number;
  fastSeconds: number;
} {
  const nowMs = params.nowMs ?? Date.now();
  const elapsedMs = Math.max(0, nowMs - params.startedAtMs);
  const fastSeconds = normalizeFastSeconds(params.fastSeconds) ?? DEFAULT_FAST_MODE_AUTO_SECONDS;
  const thresholdMs = fastSeconds * 1000;
  const enabled = params.mode === "auto" ? elapsedMs <= thresholdMs : params.mode === true;
  const elapsedSeconds = Math.floor(elapsedMs / 1000);
  return {
    mode: params.mode,
    enabled,
    elapsedSeconds,
    fastSeconds,
  };
}

export function formatFastModeAutoProgressText(params: {
  enabled: boolean;
  elapsedSeconds: number;
  fastSeconds: number;
}): string {
  if (params.enabled) {
    return "💨Fast: auto-on";
  }
  return `💨Fast: auto-off(${params.elapsedSeconds}s>${params.fastSeconds}s)`;
}

export function formatFastModeValue(mode: FastMode | undefined): "auto" | "on" | "off" {
  return mode === "auto" ? "auto" : mode === true ? "on" : "off";
}

export function formatFastModeAutoLabel(fastSeconds?: number): string {
  const seconds = normalizeFastSeconds(fastSeconds) ?? DEFAULT_FAST_MODE_AUTO_SECONDS;
  return `auto (${seconds} sec)`;
}

export function formatFastModeStatusValue(params: {
  mode: FastMode | undefined;
  fastSeconds?: number;
}): string {
  if (params.mode === "auto") {
    return formatFastModeAutoLabel(params.fastSeconds);
  }
  return formatFastModeValue(params.mode);
}

export function resolveFastModeState(params: {
  cfg: OpenClawConfig | undefined;
  provider: string;
  model: string;
  agentId?: string;
  sessionEntry?: Pick<SessionEntry, "fastMode"> | undefined;
}): FastModeState {
  const sessionOverride = normalizeFastMode(params.sessionEntry?.fastMode);
  if (sessionOverride !== undefined) {
    return {
      mode: sessionOverride,
      enabled: sessionOverride === "auto" ? true : sessionOverride,
      source: "session",
      fastSeconds: resolveConfiguredFastSeconds(params),
    };
  }

  const agentDefault =
    params.agentId && params.cfg
      ? resolveAgentConfig(params.cfg, params.agentId)?.fastModeDefault
      : undefined;
  const normalizedAgentDefault = normalizeFastMode(agentDefault);
  if (normalizedAgentDefault !== undefined) {
    return {
      mode: normalizedAgentDefault,
      enabled: normalizedAgentDefault === "auto" ? true : normalizedAgentDefault,
      source: "agent",
      fastSeconds: resolveConfiguredFastSeconds(params),
    };
  }

  const configuredRaw = resolveConfiguredFastModeRaw(params);
  const configured = normalizeFastMode(configuredRaw as string | boolean | null | undefined);
  if (configured !== undefined) {
    return {
      mode: configured,
      enabled: configured === "auto" ? true : configured,
      source: "config",
      fastSeconds: resolveConfiguredFastSeconds(params),
    };
  }

  return {
    mode: false,
    enabled: false,
    source: "default",
    fastSeconds: resolveConfiguredFastSeconds(params),
  };
}
