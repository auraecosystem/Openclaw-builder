import crypto from "node:crypto";
import { compileSafeRegexDetailed } from "../security/safe-regex.js";
import { normalizeLowercaseStringOrEmpty } from "../shared/string-coerce.js";
import { buildCommandPayloadCandidates } from "./command-analysis/risks.js";
import {
  analyzeShellCommand,
  type ExecCommandAnalysis,
  type ExecCommandSegment,
} from "./exec-approvals-analysis.js";
import type { ExecDenylistEntry } from "./exec-approvals.types.js";

export const DEFAULT_EXEC_DENYLIST_ENTRIES: readonly ExecDenylistEntry[] = [
  {
    id: "default-shell-network-fetch",
    pattern: String.raw`(?:^|[\s;&|()<>])(?:curl|wget)(?:\.exe)?(?:$|[\s;&|()<>$])|[\\/](?:curl|wget)(?:\.exe)?(?:$|[\s;&|()<>$])`,
  },
];

const MAX_DENYLIST_PATTERN_LENGTH = 8 * 1024;
export const MAX_EXEC_DENYLIST_RULES = 256;
const MAX_DENYLIST_INSPECTED_CHARS = 256 * 1024;
const ALLOWED_DENYLIST_REGEX_FLAGS = new Set(["i", "m", "u"]);

export type ExecDenylistInvalidReason =
  | "empty"
  | "invalid-flags"
  | "pattern-too-long"
  | "too-many-rules"
  | "unsafe-regex"
  | "input-too-large";

export type ExecDenylistDecision =
  | {
      denied: false;
      invalid: false;
      commandHash: string;
      commandLength: number;
    }
  | {
      denied: true;
      invalid: false;
      ruleIndex: number;
      commandHash: string;
      commandLength: number;
    }
  | {
      denied: true;
      invalid: true;
      reason: ExecDenylistInvalidReason;
      ruleIndex?: number;
      commandHash: string;
      commandLength: number;
    };

type CompiledDenyRule = {
  ruleIndex: number;
  regex: RegExp;
};

type EnvReference = {
  name: string;
  caseInsensitive: boolean;
};

function commandHash(command: string): string {
  return crypto.createHash("sha256").update(command).digest("hex").slice(0, 16);
}

function normalizeFlags(flags?: unknown): string | null {
  if (flags !== undefined && typeof flags !== "string") {
    return null;
  }
  const raw = flags?.trim() ?? "";
  const unique = [...new Set(raw.split(""))].join("");
  for (const flag of unique) {
    if (!ALLOWED_DENYLIST_REGEX_FLAGS.has(flag)) {
      return null;
    }
  }
  return unique;
}

function compileDenyRules(denylist: readonly ExecDenylistEntry[]):
  | { ok: true; rules: CompiledDenyRule[] }
  | {
      ok: false;
      reason: ExecDenylistInvalidReason;
      ruleIndex?: number;
    } {
  const rules: CompiledDenyRule[] = [];
  if (denylist.length > MAX_EXEC_DENYLIST_RULES) {
    return { ok: false, reason: "too-many-rules" };
  }
  for (let idx = 0; idx < denylist.length; idx += 1) {
    const entry = denylist[idx];
    if (typeof entry?.pattern !== "string") {
      return { ok: false, reason: "empty", ruleIndex: idx };
    }
    const pattern = entry.pattern.trim();
    if (!pattern) {
      return { ok: false, reason: "empty", ruleIndex: idx };
    }
    if (pattern.length > MAX_DENYLIST_PATTERN_LENGTH) {
      return { ok: false, reason: "pattern-too-long", ruleIndex: idx };
    }
    const flags = normalizeFlags(entry.flags);
    if (flags === null) {
      return { ok: false, reason: "invalid-flags", ruleIndex: idx };
    }
    const compiled = compileSafeRegexDetailed(pattern, flags);
    if (!compiled.regex) {
      return { ok: false, reason: "unsafe-regex", ruleIndex: idx };
    }
    rules.push({ ruleIndex: idx, regex: compiled.regex });
  }
  return { ok: true, rules };
}

function pushCandidate(candidates: string[], value: string | undefined | null): void {
  const trimmed = value?.trim();
  if (trimmed) {
    candidates.push(trimmed);
  }
}

function pushLines(candidates: string[], value: string): void {
  for (const line of value.split(/\r?\n/u)) {
    pushCandidate(candidates, line);
  }
}

function collectEnvReferences(command: string): EnvReference[] {
  const references: EnvReference[] = [];
  const seen = new Set<string>();
  const pushReference = (name: string | undefined, caseInsensitive: boolean) => {
    if (!name) {
      return;
    }
    const key = `${caseInsensitive ? "i" : "s"}\0${name}`;
    if (seen.has(key)) {
      return;
    }
    seen.add(key);
    references.push({ name, caseInsensitive });
  };
  for (const match of command.matchAll(
    /\$(?:\{([A-Za-z_][A-Za-z0-9_]*)\}|([A-Za-z_][A-Za-z0-9_]*))/gu,
  )) {
    pushReference(match[1] ?? match[2], false);
  }
  for (const match of command.matchAll(
    /\$(?:env:([A-Za-z_][A-Za-z0-9_]*)|\{env:([A-Za-z_][A-Za-z0-9_]*)\})/giu,
  )) {
    pushReference(match[1] ?? match[2], true);
  }
  for (const match of command.matchAll(
    /%(?:([A-Za-z_][A-Za-z0-9_]*))%|!(?:([A-Za-z_][A-Za-z0-9_]*))!/gu,
  )) {
    pushReference(match[1] ?? match[2], true);
  }
  return references;
}

function resolveEnvReference(
  env: NodeJS.ProcessEnv | undefined,
  reference: EnvReference,
): string | undefined {
  const exact = env?.[reference.name];
  if (exact !== undefined || !reference.caseInsensitive || !env) {
    return exact;
  }
  const normalizedName = reference.name.toLowerCase();
  for (const [key, value] of Object.entries(env)) {
    if (key.toLowerCase() === normalizedName) {
      return value;
    }
  }
  return undefined;
}

function pushSegmentCandidates(candidates: string[], segment: ExecCommandSegment): void {
  pushCandidate(candidates, segment.raw);
  for (const arg of segment.argv) {
    pushCandidate(candidates, arg);
  }
  const effectiveArgv = segment.resolution?.effectiveArgv;
  if (effectiveArgv) {
    for (const arg of effectiveArgv) {
      pushCandidate(candidates, arg);
    }
  }
  pushCandidate(candidates, segment.resolution?.execution.executableName);
  pushCandidate(candidates, segment.resolution?.execution.rawExecutable);
  pushCandidate(candidates, segment.resolution?.execution.resolvedPath);
  pushCandidate(candidates, segment.resolution?.execution.resolvedRealPath);
  pushCandidate(candidates, segment.resolution?.policy.executableName);
  pushCandidate(candidates, segment.resolution?.policy.rawExecutable);
  pushCandidate(candidates, segment.resolution?.policy.resolvedPath);
  pushCandidate(candidates, segment.resolution?.policy.resolvedRealPath);
  for (const payload of buildCommandPayloadCandidates(effectiveArgv ?? segment.argv)) {
    pushCandidate(candidates, payload);
    pushLines(candidates, payload);
  }
}

function collectDenylistCandidates(params: {
  command: string;
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  analysis?: ExecCommandAnalysis;
}): string[] {
  const candidates: string[] = [];
  pushCandidate(candidates, params.command);
  pushLines(candidates, params.command);
  pushCandidate(candidates, normalizeLowercaseStringOrEmpty(params.command));

  const analysis =
    params.analysis ??
    analyzeShellCommand({
      command: params.command,
      cwd: params.cwd,
      env: params.env,
      platform: process.platform,
    });
  for (const segment of analysis.segments) {
    pushSegmentCandidates(candidates, segment);
  }

  const referencedEnv = collectEnvReferences(params.command);
  for (const reference of referencedEnv) {
    const value = resolveEnvReference(params.env, reference);
    pushCandidate(candidates, value);
    pushCandidate(candidates, normalizeLowercaseStringOrEmpty(value));
  }

  return [...new Set(candidates)];
}

function testDenyRule(regex: RegExp, candidate: string): boolean {
  regex.lastIndex = 0;
  return regex.test(candidate);
}

export function evaluateExecDenylist(params: {
  command: string;
  denylist: readonly ExecDenylistEntry[];
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  analysis?: ExecCommandAnalysis;
}): ExecDenylistDecision {
  const base = {
    commandHash: commandHash(params.command),
    commandLength: params.command.length,
  };
  if (params.denylist.length === 0) {
    return { denied: false, invalid: false, ...base };
  }
  const compiled = compileDenyRules(params.denylist);
  if (!compiled.ok) {
    return {
      denied: true,
      invalid: true,
      reason: compiled.reason,
      ruleIndex: compiled.ruleIndex,
      ...base,
    };
  }
  const candidates = collectDenylistCandidates(params);
  const inspectedChars = candidates.reduce((total, candidate) => total + candidate.length, 0);
  if (inspectedChars > MAX_DENYLIST_INSPECTED_CHARS) {
    return { denied: true, invalid: true, reason: "input-too-large", ...base };
  }
  for (const candidate of candidates) {
    for (const rule of compiled.rules) {
      if (testDenyRule(rule.regex, candidate)) {
        return { denied: true, invalid: false, ruleIndex: rule.ruleIndex, ...base };
      }
    }
  }
  return { denied: false, invalid: false, ...base };
}
