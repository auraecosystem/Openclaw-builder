import { normalizeFastMode } from "../auto-reply/thinking.shared.js";
import type { SessionEntry } from "../config/sessions.js";
import type { OpenClawConfig } from "../config/types.openclaw.js";
import { DEFAULT_FAST_MODE_AUTO_SECONDS, resolveFastModeModelParams } from "../shared/fast-mode.js";
import type { FastMode } from "../shared/string-coerce.js";
import { resolveAgentConfig } from "./agent-scope.js";

export {
  DEFAULT_FAST_MODE_AUTO_SECONDS,
  formatFastModeAutoLabel,
  formatFastModeAutoProgressText,
  formatFastModeStatusValue,
  formatFastModeValue,
} from "../shared/fast-mode.js";
export type { FastMode } from "../shared/string-coerce.js";

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
  const modelParams = resolveFastModeModelParams(params);
  return modelParams?.fastMode ?? modelParams?.fast_mode;
}

export function resolveFastModeForElapsed(params: {
  mode?: FastMode;
  startedAtMs: number;
  nowMs?: number;
}): {
  mode: FastMode | undefined;
  enabled: boolean;
  elapsedSeconds: number;
} {
  const nowMs = params.nowMs ?? Date.now();
  const elapsedMs = Math.max(0, nowMs - params.startedAtMs);
  const thresholdMs = DEFAULT_FAST_MODE_AUTO_SECONDS * 1000;
  const enabled = params.mode === "auto" ? elapsedMs <= thresholdMs : params.mode === true;
  const elapsedSeconds = Math.floor(elapsedMs / 1000);
  return {
    mode: params.mode,
    enabled,
    elapsedSeconds,
  };
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
      fastSeconds: DEFAULT_FAST_MODE_AUTO_SECONDS,
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
      fastSeconds: DEFAULT_FAST_MODE_AUTO_SECONDS,
    };
  }

  const configuredRaw = resolveConfiguredFastModeRaw(params);
  const configured = normalizeFastMode(configuredRaw as string | boolean | null | undefined);
  if (configured !== undefined) {
    return {
      mode: configured,
      enabled: configured === "auto" ? true : configured,
      source: "config",
      fastSeconds: DEFAULT_FAST_MODE_AUTO_SECONDS,
    };
  }

  return {
    mode: false,
    enabled: false,
    source: "default",
    fastSeconds: DEFAULT_FAST_MODE_AUTO_SECONDS,
  };
}
