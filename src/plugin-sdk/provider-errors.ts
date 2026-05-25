import type {
  ProviderErrorAction,
  ProviderErrorClassification,
  ProviderErrorDescriptor,
  ProviderFailoverErrorContext,
} from "../plugins/types.js";

export type {
  ProviderErrorAction,
  ProviderErrorActionKind,
  ProviderErrorClassification,
  ProviderErrorDescriptor,
  ProviderFailoverErrorContext,
} from "../plugins/types.js";

export type ProviderErrorMapResolver<T> = (ctx: ProviderFailoverErrorContext) => T | undefined;

export type ProviderErrorMapValue<T> = T | ProviderErrorMapResolver<T>;

export type ProviderErrorMapEntry = {
  codes?: readonly string[];
  status?: number | readonly number[];
  messagePatterns?: readonly RegExp[];
  reason: ProviderErrorDescriptor["reason"];
  userMessage?: ProviderErrorMapValue<string>;
  retryAfterMs?: ProviderErrorMapValue<number>;
  action?: ProviderErrorMapValue<ProviderErrorAction>;
};

export type ProviderErrorClassifier = (
  ctx: ProviderFailoverErrorContext,
) => ProviderErrorClassification | undefined;

const CODE_SEPARATOR_RE = /[^A-Z0-9]+/gu;
const EDGE_SEPARATOR_RE = /^_+|_+$/gu;
const STRUCTURED_CODE_SIGNAL_RE =
  /\b(?:code|type|error_code|error_type)["']?\s*[:=]\s*["']?([A-Za-z0-9][A-Za-z0-9._/-]*)["']?/giu;

export function normalizeProviderErrorCodeSignal(value: string | undefined): string | undefined {
  const normalized = value
    ?.trim()
    .toUpperCase()
    .replace(CODE_SEPARATOR_RE, "_")
    .replace(EDGE_SEPARATOR_RE, "");
  return normalized || undefined;
}

type NormalizedProviderErrorCodeEntry = {
  raw: string;
  normalized: string;
};

function normalizeProviderErrorCodeEntries(
  codes: readonly string[] | undefined,
): NormalizedProviderErrorCodeEntry[] {
  return (
    codes
      ?.map((raw) => {
        const normalized = normalizeProviderErrorCodeSignal(raw);
        return normalized ? { raw, normalized } : undefined;
      })
      .filter((entry): entry is NormalizedProviderErrorCodeEntry => Boolean(entry)) ?? []
  );
}

function statusMatches(entryStatus: ProviderErrorMapEntry["status"], status: number | undefined) {
  if (entryStatus === undefined) {
    return true;
  }
  if (status === undefined) {
    return false;
  }
  return Array.isArray(entryStatus) ? entryStatus.includes(status) : entryStatus === status;
}

function messagePatternMatches(
  patterns: readonly RegExp[] | undefined,
  errorMessage: string | undefined,
) {
  if (!patterns?.length) {
    return true;
  }
  if (!errorMessage) {
    return false;
  }
  return patterns.some((pattern) => {
    pattern.lastIndex = 0;
    return pattern.test(errorMessage);
  });
}

function findMatchingProviderErrorCode(
  codes: readonly string[] | undefined,
  ctx: ProviderFailoverErrorContext,
) {
  if (!codes?.length) {
    return undefined;
  }
  const normalizedCodes = normalizeProviderErrorCodeEntries(codes);
  if (normalizedCodes.length === 0) {
    return undefined;
  }
  const directCode = normalizeProviderErrorCodeSignal(ctx.code);
  const matchingDirectCode = normalizedCodes.find((entry) => entry.normalized === directCode);
  if (matchingDirectCode) {
    return ctx.code ?? matchingDirectCode.raw;
  }
  const directType = normalizeProviderErrorCodeSignal(ctx.errorType);
  const matchingDirectType = normalizedCodes.find((entry) => entry.normalized === directType);
  if (matchingDirectType) {
    return matchingDirectType.raw;
  }
  const messageSignals = extractProviderErrorCodeSignalsFromMessage(ctx.errorMessage);
  if (messageSignals.length === 0) {
    return undefined;
  }
  return normalizedCodes.find((entry) => messageSignals.includes(entry.normalized))?.raw;
}

function extractProviderErrorCodeSignalsFromMessage(errorMessage: string): string[] {
  const signals: string[] = [];
  for (const match of errorMessage.matchAll(STRUCTURED_CODE_SIGNAL_RE)) {
    const normalized = normalizeProviderErrorCodeSignal(match[1]);
    if (normalized) {
      signals.push(normalized);
    }
  }
  return signals;
}

function hasMatcher(entry: ProviderErrorMapEntry) {
  return Boolean(
    entry.codes?.length || entry.messagePatterns?.length || entry.status !== undefined,
  );
}

function resolveProviderErrorMapValue<T>(
  value: ProviderErrorMapValue<T> | undefined,
  ctx: ProviderFailoverErrorContext,
) {
  if (typeof value !== "function") {
    return value;
  }
  return (value as ProviderErrorMapResolver<T>)(ctx);
}

export function classifyProviderErrorFromMap(
  ctx: ProviderFailoverErrorContext,
  entries: readonly ProviderErrorMapEntry[],
): ProviderErrorDescriptor | undefined {
  for (const entry of entries) {
    if (!hasMatcher(entry)) {
      continue;
    }
    if (!statusMatches(entry.status, ctx.status)) {
      continue;
    }
    const matchingCode = findMatchingProviderErrorCode(entry.codes, ctx);
    if (entry.codes?.length && !matchingCode) {
      continue;
    }
    if (!messagePatternMatches(entry.messagePatterns, ctx.errorMessage)) {
      continue;
    }
    const userMessage = resolveProviderErrorMapValue(entry.userMessage, ctx);
    const retryAfterMs = resolveProviderErrorMapValue(entry.retryAfterMs, ctx);
    const action = resolveProviderErrorMapValue(entry.action, ctx);
    return {
      reason: entry.reason,
      ...(ctx.status !== undefined ? { status: ctx.status } : {}),
      ...(matchingCode ? { code: matchingCode } : {}),
      ...(ctx.errorType ? { errorType: ctx.errorType } : {}),
      ...(userMessage ? { userMessage } : {}),
      ...(retryAfterMs !== undefined ? { retryAfterMs } : {}),
      ...(action ? { action } : {}),
    };
  }
  return undefined;
}

export function defineProviderErrorMap(
  entries: readonly ProviderErrorMapEntry[],
): ProviderErrorClassifier {
  return (ctx) => classifyProviderErrorFromMap(ctx, entries);
}
