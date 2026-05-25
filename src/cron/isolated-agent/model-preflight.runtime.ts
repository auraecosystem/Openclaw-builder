import { normalizeProviderId } from "../../agents/provider-id.js";
import type { ModelProviderConfig } from "../../config/types.models.js";
import type { OpenClawConfig } from "../../config/types.openclaw.js";
import { fetchWithSsrFGuard } from "../../infra/net/fetch-guard.js";
import type { SsrFPolicy } from "../../infra/net/ssrf.js";
import { normalizeLowercaseStringOrEmpty } from "../../shared/string-coerce.js";

const PREFLIGHT_CACHE_TTL_MS = 5 * 60_000;
const PREFLIGHT_TIMEOUT_MS = 2_500;
const PREFLIGHT_MAX_ATTEMPTS = 1;
const PREFLIGHT_RETRY_DELAY_MS = 0;

type PreflightApi = "ollama" | "openai-completions";

export type CronModelProviderPreflightResult =
  | { status: "available" }
  | {
      status: "unavailable";
      reason: string;
      provider: string;
      model: string;
      baseUrl: string;
      retryAfterMs: number;
    };

type EndpointPreflightResult =
  | { status: "available" }
  | {
      status: "unavailable";
      error: unknown;
      attempts: number;
    };

type CachedEndpointPreflightResult = {
  checkedAtMs: number;
  result: EndpointPreflightResult;
};

const preflightCache = new Map<string, CachedEndpointPreflightResult>();

function resolveProviderConfig(
  cfg: OpenClawConfig,
  provider: string,
): ModelProviderConfig | undefined {
  const providers = cfg.models?.providers;
  if (!providers) {
    return undefined;
  }
  const direct = providers[provider];
  if (direct) {
    return direct;
  }
  const normalized = normalizeProviderId(provider);
  return Object.entries(providers).find(([key]) => normalizeProviderId(key) === normalized)?.[1];
}

function normalizeBaseUrl(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const trimmed = value.trim().replace(/\/+$/, "");
  return trimmed ? trimmed : undefined;
}

function normalizeProbeApi(providerConfig: ModelProviderConfig): PreflightApi | undefined {
  const api = normalizeLowercaseStringOrEmpty(providerConfig.api);
  return api === "ollama" || api === "openai-completions" ? api : undefined;
}

function isPrivateIpv4Host(host: string): boolean {
  if (!/^\d+\.\d+\.\d+\.\d+$/.test(host)) {
    return false;
  }
  const octets = host.split(".").map((part) => Number.parseInt(part, 10));
  if (octets.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return false;
  }
  const [a, b] = octets;
  return a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168);
}

function isLocalProviderBaseUrl(baseUrl: string): boolean {
  try {
    let host = normalizeLowercaseStringOrEmpty(new URL(baseUrl).hostname);
    if (host.startsWith("[") && host.endsWith("]")) {
      host = host.slice(1, -1);
    }
    return (
      host === "localhost" ||
      host === "127.0.0.1" ||
      host === "0.0.0.0" ||
      host === "::1" ||
      host === "::ffff:7f00:1" ||
      host === "::ffff:127.0.0.1" ||
      host.endsWith(".local") ||
      isPrivateIpv4Host(host)
    );
  } catch {
    return false;
  }
}

function buildProbeUrl(api: PreflightApi, baseUrl: string): string {
  if (api === "ollama") {
    return `${baseUrl}/api/tags`;
  }
  return `${baseUrl}/models`;
}

function buildLocalProviderSsrFPolicy(baseUrl: string): SsrFPolicy | undefined {
  try {
    const parsed = new URL(baseUrl);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return undefined;
    }
    return {
      hostnameAllowlist: [parsed.hostname],
      allowPrivateNetwork: true,
    };
  } catch {
    return undefined;
  }
}

function formatUnavailableReason(params: {
  provider: string;
  model: string;
  baseUrl: string;
  error: unknown;
  attempts: number;
}): string {
  return [
    `Agent cron job uses ${params.provider}/${params.model} but the local provider endpoint is not reachable at ${params.baseUrl}.`,
    `Skipping this cron run after ${params.attempts} preflight attempt${params.attempts === 1 ? "" : "s"}; OpenClaw will retry the provider preflight on a later scheduled run.`,
    `Last error: ${String(params.error)}`,
  ].join(" ");
}

function buildUnavailableResult(params: {
  provider: string;
  model: string;
  baseUrl: string;
  error: unknown;
  attempts: number;
}): CronModelProviderPreflightResult {
  return {
    status: "unavailable",
    provider: params.provider,
    model: params.model,
    baseUrl: params.baseUrl,
    retryAfterMs: PREFLIGHT_CACHE_TTL_MS,
    reason: formatUnavailableReason({
      provider: params.provider,
      model: params.model,
      baseUrl: params.baseUrl,
      error: params.error,
      attempts: params.attempts,
    }),
  };
}

function normalizePositiveInteger(value: unknown, fallback: number): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) {
    return fallback;
  }
  return value;
}

function normalizeNonNegativeInteger(value: unknown, fallback: number): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    return fallback;
  }
  return value;
}

function sleepMs(delayMs: number): Promise<void> {
  if (delayMs <= 0) {
    return Promise.resolve();
  }
  return new Promise((resolve) => setTimeout(resolve, delayMs));
}

async function probeLocalProviderEndpoint(params: {
  api: PreflightApi;
  baseUrl: string;
  timeoutMs: number;
}): Promise<void> {
  const { response, release } = await fetchWithSsrFGuard({
    url: buildProbeUrl(params.api, params.baseUrl),
    init: { method: "GET" },
    policy: buildLocalProviderSsrFPolicy(params.baseUrl),
    timeoutMs: params.timeoutMs,
    auditContext: "cron-model-provider-preflight",
  });
  try {
    // Any HTTP response means the local endpoint is alive. Auth/model errors
    // still belong to the normal model runner where fallback and diagnostics
    // have the full provider context.
    void response.status;
  } finally {
    await release();
  }
}

export async function preflightCronModelProvider(params: {
  cfg: OpenClawConfig;
  provider: string;
  model: string;
  nowMs?: number;
}): Promise<CronModelProviderPreflightResult> {
  const providerConfig = resolveProviderConfig(params.cfg, params.provider);
  if (!providerConfig) {
    return { status: "available" };
  }
  const baseUrl = normalizeBaseUrl(providerConfig.baseUrl);
  const api = normalizeProbeApi(providerConfig);
  if (!baseUrl || !api || !isLocalProviderBaseUrl(baseUrl)) {
    return { status: "available" };
  }
  const preflightConfig = params.cfg.cron?.modelPreflight;
  const timeoutMs = normalizePositiveInteger(preflightConfig?.timeoutMs, PREFLIGHT_TIMEOUT_MS);
  const maxAttempts = normalizePositiveInteger(
    preflightConfig?.maxAttempts,
    PREFLIGHT_MAX_ATTEMPTS,
  );
  const retryDelayMs = normalizeNonNegativeInteger(
    preflightConfig?.retryDelayMs,
    PREFLIGHT_RETRY_DELAY_MS,
  );

  const nowMs = params.nowMs ?? Date.now();
  const cacheKey = `${api}\0${baseUrl}`;
  const cached = preflightCache.get(cacheKey);
  if (cached && nowMs - cached.checkedAtMs < PREFLIGHT_CACHE_TTL_MS) {
    if (cached.result.status === "available") {
      return { status: "available" };
    }
    return buildUnavailableResult({
      provider: params.provider,
      model: params.model,
      baseUrl,
      error: cached.result.error,
      attempts: cached.result.attempts,
    });
  }

  let result: EndpointPreflightResult;
  let lastError: unknown;
  let attempts = 0;
  for (attempts = 1; attempts <= maxAttempts; attempts += 1) {
    try {
      await probeLocalProviderEndpoint({ api, baseUrl, timeoutMs });
      result = { status: "available" };
      preflightCache.set(cacheKey, { checkedAtMs: nowMs, result });
      return { status: "available" };
    } catch (error) {
      lastError = error;
      if (attempts < maxAttempts) {
        await sleepMs(retryDelayMs);
      }
    }
  }
  result = { status: "unavailable", error: lastError, attempts: maxAttempts };
  preflightCache.set(cacheKey, { checkedAtMs: nowMs, result });
  return buildUnavailableResult({
    provider: params.provider,
    model: params.model,
    baseUrl,
    error: result.error,
    attempts: result.attempts,
  });
}

export function resetCronModelProviderPreflightCacheForTest(): void {
  preflightCache.clear();
}
