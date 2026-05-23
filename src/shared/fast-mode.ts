import type { FastMode } from "./string-coerce.js";
import { normalizeLowercaseStringOrEmpty } from "./string-coerce.js";

export const DEFAULT_FAST_MODE_AUTO_ON_SECONDS = 60;

export type FastModeSource = "session" | "agent" | "config" | "default";

type FastModeModelConfig = {
  params?: Record<string, unknown>;
};

type FastModeConfig = {
  agents?: {
    defaults?: {
      models?: Record<string, FastModeModelConfig | undefined>;
    };
  };
};

function modelConfigKey(provider?: string, model?: string): string {
  const providerId = provider?.trim() ?? "";
  const modelId = model?.trim() ?? "";
  if (!providerId) {
    return modelId;
  }
  if (!modelId) {
    return providerId;
  }
  return normalizeLowercaseStringOrEmpty(modelId).startsWith(
    `${normalizeLowercaseStringOrEmpty(providerId)}/`,
  )
    ? modelId
    : `${providerId}/${modelId}`;
}

export function resolveFastModeModelParams(params: {
  cfg: FastModeConfig | undefined;
  provider?: string;
  model?: string;
}): Record<string, unknown> | undefined {
  const modelConfig =
    params.cfg?.agents?.defaults?.models?.[modelConfigKey(params.provider, params.model)];
  return modelConfig?.params;
}

export function normalizeFastModeAutoOnSeconds(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) && value > 0 ? value : undefined;
}

export function resolveFastModeModelAutoOnSeconds(params: {
  cfg: FastModeConfig | undefined;
  provider?: string;
  model?: string;
}): number {
  const modelParams = resolveFastModeModelParams(params);
  return (
    normalizeFastModeAutoOnSeconds(modelParams?.fastAutoOnSeconds) ??
    DEFAULT_FAST_MODE_AUTO_ON_SECONDS
  );
}

export function formatFastModeAutoProgressText(params: {
  enabled: boolean;
  elapsedSeconds: number;
  fastAutoOnSeconds?: number;
}): string {
  if (params.enabled) {
    return "💨Fast: auto-on";
  }
  const fastAutoOnSeconds =
    normalizeFastModeAutoOnSeconds(params.fastAutoOnSeconds) ?? DEFAULT_FAST_MODE_AUTO_ON_SECONDS;
  return `💨Fast: auto-off(${params.elapsedSeconds}s>=${fastAutoOnSeconds}s)`;
}

export function formatFastModeValue(mode: FastMode | undefined): "auto" | "on" | "off" {
  return mode === "auto" ? "auto" : mode === true ? "on" : "off";
}

export function formatFastModeAutoLabel(params?: { fastAutoOnSeconds?: number }): string {
  const fastAutoOnSeconds =
    normalizeFastModeAutoOnSeconds(params?.fastAutoOnSeconds) ?? DEFAULT_FAST_MODE_AUTO_ON_SECONDS;
  return `auto (${fastAutoOnSeconds} sec)`;
}

export function formatFastModeStatusValue(params: {
  mode: FastMode | undefined;
  fastAutoOnSeconds?: number;
}): string {
  if (params.mode === "auto") {
    return formatFastModeAutoLabel({ fastAutoOnSeconds: params.fastAutoOnSeconds });
  }
  return formatFastModeValue(params.mode);
}

export function formatFastModeSourceSuffix(source: FastModeSource | undefined): string {
  switch (source) {
    case "session":
      return " (session)";
    case "agent":
      return " (default: agent)";
    case "config":
      return " (default: model)";
    case "default":
      return " (default)";
    default:
      return "";
  }
}
