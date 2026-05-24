type JsonRecord = Record<string, unknown>;

const PROFILER_FLAGS = ["profiler", "codex.profiler"] as const;
const DIAGNOSTICS_ENV = "OPENCLAW_DIAGNOSTICS";

function asRecord(value: unknown): JsonRecord | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonRecord)
    : undefined;
}

function normalizeFlag(value: unknown): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const normalized = value.trim().toLowerCase();
  return normalized || undefined;
}

function readConfigDiagnosticFlags(config: unknown): string[] {
  const diagnostics = asRecord(asRecord(config)?.diagnostics);
  const flags = diagnostics?.flags;
  return Array.isArray(flags)
    ? flags.map(normalizeFlag).filter((flag): flag is string => !!flag)
    : [];
}

function readEnvDiagnosticFlags(env: NodeJS.ProcessEnv): string[] {
  const normalized = normalizeFlag(env[DIAGNOSTICS_ENV]);
  if (!normalized || ["0", "false", "off", "none"].includes(normalized)) {
    return [];
  }
  if (["1", "true", "all", "*"].includes(normalized)) {
    return ["*"];
  }
  return normalized
    .split(/[,\s]+/u)
    .map(normalizeFlag)
    .filter((flag): flag is string => !!flag);
}

function matchesFlag(target: string, enabled: string): boolean {
  if (enabled === "*" || enabled === "all") {
    return true;
  }
  if (enabled.endsWith(".*")) {
    const prefix = enabled.slice(0, -2);
    return target === prefix || target.startsWith(`${prefix}.`);
  }
  if (enabled.endsWith("*")) {
    return target.startsWith(enabled.slice(0, -1));
  }
  return enabled === target;
}

export function isCodexAppServerProfilerEnabled(
  config?: unknown,
  env: NodeJS.ProcessEnv = process.env,
): boolean {
  const flags = [...readConfigDiagnosticFlags(config), ...readEnvDiagnosticFlags(env)];
  return PROFILER_FLAGS.some((target) => flags.some((enabled) => matchesFlag(target, enabled)));
}
