import { normalizeLowercaseStringOrEmpty } from "../shared/string-coerce.js";

export function normalizeProviderId(provider: string): string {
  const normalized = normalizeLowercaseStringOrEmpty(provider);
  if (normalized === "modelstudio" || normalized === "qwencloud") {
    return "qwen";
  }
  if (normalized === "z.ai" || normalized === "z-ai") {
    return "zai";
  }
  if (normalized === "opencode-zen") {
    return "opencode";
  }
  if (normalized === "opencode-go-auth") {
    return "opencode-go";
  }
  if (normalized === "anthropic-cli") {
    return "claude-cli";
  }
  if (normalized === "kimi" || normalized === "kimi-code" || normalized === "kimi-coding") {
    return "kimi";
  }
  if (normalized === "moonshotai" || normalized === "moonshot-ai") {
    return "moonshot";
  }
  if (normalized === "bedrock" || normalized === "aws-bedrock") {
    return "amazon-bedrock";
  }
  // Backward compatibility for older provider naming.
  if (normalized === "bytedance" || normalized === "doubao") {
    return "volcengine";
  }
  return normalized;
}

/** Normalize provider ID before manifest-owned auth alias lookup. */
export function normalizeProviderIdForAuth(provider: string): string {
  return normalizeProviderId(provider);
}

// True for any provider id that maps to a Claude CLI subscription backend —
// today the headless `claude-cli` and the interactive proxy `claude-cli-interactive`.
// Both spawn the same `claude` binary against Anthropic's subscription auth,
// so any "is this a Claude CLI run?" gate (model-capability resolution, OAuth
// label lookup, credential fingerprinting, live-agent probe routing, etc.)
// should treat them as the same family. Single helper so future variants get
// recognised everywhere by extending this one function instead of grepping
// for every `=== "claude-cli"` site.
export function isClaudeCliCompatibleBackend(provider: string | undefined): boolean {
  const normalized = normalizeProviderId(provider ?? "");
  return normalized === "claude-cli" || normalized === "claude-cli-interactive";
}

export function findNormalizedProviderValue<T>(
  entries: Record<string, T> | undefined,
  provider: string,
): T | undefined {
  if (!entries) {
    return undefined;
  }
  const providerKey = normalizeProviderId(provider);
  for (const [key, value] of Object.entries(entries)) {
    if (normalizeProviderId(key) === providerKey) {
      return value;
    }
  }
  return undefined;
}

export function findNormalizedProviderKey(
  entries: Record<string, unknown> | undefined,
  provider: string,
): string | undefined {
  if (!entries) {
    return undefined;
  }
  const providerKey = normalizeProviderId(provider);
  return Object.keys(entries).find((key) => normalizeProviderId(key) === providerKey);
}
