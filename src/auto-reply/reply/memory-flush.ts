import crypto from "node:crypto";
import { resolveContextTokensForModel } from "../../agents/context.js";
import { DEFAULT_CONTEXT_TOKENS } from "../../agents/defaults.js";
import { resolveFreshSessionTotalTokens, type SessionEntry } from "../../config/sessions.js";
import type { OpenClawConfig } from "../../config/types.openclaw.js";

export function resolveMemoryFlushContextWindowTokens(params: {
  modelId?: string;
  agentCfgContextTokens?: number;
  cfg?: OpenClawConfig;
  provider?: string;
}): number {
  return (
    resolveContextTokensForModel({
      cfg: params.cfg,
      provider: params.provider,
      model: params.modelId,
      contextTokensOverride: params.agentCfgContextTokens,
      allowAsyncLoad: false,
    }) ?? DEFAULT_CONTEXT_TOKENS
  );
}

function resolvePositiveTokenCount(value: number | undefined): number | undefined {
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.floor(value)
    : undefined;
}

function resolveMemoryFlushGateState<
  TEntry extends Pick<SessionEntry, "totalTokens" | "totalTokensFresh">,
>(params: {
  entry?: TEntry;
  tokenCount?: number;
  contextWindowTokens: number;
  reserveTokensFloor: number;
  softThresholdTokens: number;
}): { entry: TEntry; totalTokens: number; threshold: number } | null {
  if (!params.entry) {
    return null;
  }

  const totalTokens =
    resolvePositiveTokenCount(params.tokenCount) ?? resolveFreshSessionTotalTokens(params.entry);
  if (!totalTokens || totalTokens <= 0) {
    return null;
  }

  const contextWindow = Math.max(1, Math.floor(params.contextWindowTokens));
  const reserveTokens = Math.max(0, Math.floor(params.reserveTokensFloor));
  const softThreshold = Math.max(0, Math.floor(params.softThresholdTokens));
  const threshold = Math.max(0, contextWindow - reserveTokens - softThreshold);
  if (threshold <= 0) {
    return null;
  }

  return { entry: params.entry, totalTokens, threshold };
}

export function shouldRunMemoryFlush(params: {
  entry?: Pick<
    SessionEntry,
    | "totalTokens"
    | "totalTokensFresh"
    | "compactionCount"
    | "memoryFlushCompactionCount"
    | "memoryFlushAt"
  >;
  /**
   * Optional token count override for flush gating. When provided, this value is
   * treated as a fresh context snapshot and used instead of the cached
   * SessionEntry.totalTokens (which may be stale/unknown).
   */
  tokenCount?: number;
  contextWindowTokens: number;
  reserveTokensFloor: number;
  softThresholdTokens: number;
}): boolean {
  const state = resolveMemoryFlushGateState(params);
  if (!state || state.totalTokens < state.threshold) {
    return false;
  }

  if (hasAlreadyFlushedForCurrentCompaction(state.entry)) {
    return false;
  }

  return true;
}

export function shouldRunPreflightCompaction(params: {
  entry?: Pick<SessionEntry, "totalTokens" | "totalTokensFresh">;
  /**
   * Optional projected token count override for pre-run compaction gating.
   * When provided, this value is treated as a fresh estimate and used instead
   * of any cached SessionEntry total.
   */
  tokenCount?: number;
  contextWindowTokens: number;
  reserveTokensFloor: number;
  softThresholdTokens: number;
}): boolean {
  const state = resolveMemoryFlushGateState(params);
  return Boolean(state && state.totalTokens >= state.threshold);
}

/**
 * Returns true when a memory flush has already been performed for the current
 * compaction cycle. This prevents repeated flush runs within the same cycle —
 * important for both the token-based and transcript-size–based trigger paths.
 *
 * Edge case: when both `compactionCount` and `memoryFlushCompactionCount` are 0
 * (or absent), we additionally require `memoryFlushAt` to be set before treating
 * the session as already flushed. This disambiguates persisted legacy rows that
 * were serialised with an implicit zero from sessions where a real flush already
 * ran in cycle 0. Fresh sessions always clear `memoryFlushAt` on creation, so
 * they correctly see `false` here until the first actual flush.
 */
export function hasAlreadyFlushedForCurrentCompaction(
  entry: Pick<SessionEntry, "compactionCount" | "memoryFlushCompactionCount" | "memoryFlushAt">,
): boolean {
  const compactionCount = entry.compactionCount ?? 0;
  const lastFlushAt = entry.memoryFlushCompactionCount;
  if (typeof lastFlushAt !== "number" || lastFlushAt !== compactionCount) {
    return false;
  }
  // When the compaction counter is 0 on both sides, the match is ambiguous:
  // it could be a legacy/never-flushed row whose field was written as 0
  // rather than left undefined. Require an explicit memoryFlushAt timestamp
  // to confirm that a real flush actually ran in this cycle.
  if (compactionCount === 0 && !entry.memoryFlushAt) {
    return false;
  }
  return true;
}

/**
 * Compute a lightweight content hash from the tail of a session transcript.
 * Used for state-based flush deduplication — if the hash hasn't changed since
 * the last flush, the context is effectively the same and flushing again would
 * produce duplicate memory entries.
 *
 * Hash input: `messages.length` + content of the last 3 user/assistant messages.
 * Algorithm: SHA-256 truncated to 16 hex chars (collision-resistant enough for dedup).
 */
export function computeContextHash(messages: Array<{ role?: string; content?: unknown }>): string {
  const userAssistant = messages.filter((m) => m.role === "user" || m.role === "assistant");
  const tail = userAssistant.slice(-3);
  const payload = `${messages.length}:${tail.map((m, i) => `[${i}:${m.role ?? ""}]${typeof m.content === "string" ? m.content : JSON.stringify(m.content ?? "")}`).join("\x00")}`;
  const hash = crypto.createHash("sha256").update(payload).digest("hex");
  return hash.slice(0, 16);
}
