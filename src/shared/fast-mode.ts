import type { FastMode } from "./string-coerce.js";
import { normalizeLowercaseStringOrEmpty } from "./string-coerce.js";

export const DEFAULT_FAST_MODE_AUTO_SECONDS = 60;

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

function modelConfigKey(provider: string, model: string): string {
  const providerId = provider.trim();
  const modelId = model.trim();
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
  provider: string;
  model: string;
}): Record<string, unknown> | undefined {
  const modelConfig =
    params.cfg?.agents?.defaults?.models?.[modelConfigKey(params.provider, params.model)];
  return modelConfig?.params;
}

export function formatFastModeAutoProgressText(params: {
  enabled: boolean;
  elapsedSeconds: number;
}): string {
  if (params.enabled) {
    return "💨Fast: auto-on";
  }
  return `💨Fast: auto-off(${params.elapsedSeconds}s>=${DEFAULT_FAST_MODE_AUTO_SECONDS}s)`;
}

export function formatFastModeValue(mode: FastMode | undefined): "auto" | "on" | "off" {
  return mode === "auto" ? "auto" : mode === true ? "on" : "off";
}

export function formatFastModeAutoLabel(): string {
  return `auto (${DEFAULT_FAST_MODE_AUTO_SECONDS} sec)`;
}

export function formatFastModeStatusValue(params: { mode: FastMode | undefined }): string {
  if (params.mode === "auto") {
    return formatFastModeAutoLabel();
  }
  return formatFastModeValue(params.mode);
}
