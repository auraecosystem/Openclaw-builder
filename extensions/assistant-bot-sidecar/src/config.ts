import path from "node:path";

export type AssistantBotSidecarConfig = {
  enabled: boolean;
  cwd: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  restartDelayMs: number;
  shutdownGraceMs: number;
  watch: {
    enabled: boolean;
    debounceMs: number;
    paths: string[];
  };
};

const DEFAULT_TRADING_TOOLS_DIR = "/Users/ad/work/trading-tools";
const DEFAULT_WATCH_PATHS = [
  "/Users/ad/work/trading-tools/apps/assistant_bot/src",
  "/Users/ad/work/trading-tools/apps/assistant_bot/pyproject.toml",
  "/Users/ad/work/trading-tools/packages/trade_journal/src",
  "/Users/ad/work/trading-tools/packages/trade_core/src",
];

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function asStringArray(value: unknown): string[] | null {
  if (!Array.isArray(value)) {
    return null;
  }
  const items = value
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean);
  return items.length > 0 ? items : null;
}

function asNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : fallback;
}

function normalizeEnv(value: unknown): Record<string, string> {
  const raw = asRecord(value);
  const entries = Object.entries(raw)
    .filter(([, entryValue]) => typeof entryValue === "string")
    .map(([key, entryValue]) => [key, String(entryValue)] as const);
  return Object.fromEntries(entries);
}

function resolvePathList(
  paths: string[],
  resolvePath: ((input: string) => string) | undefined,
): string[] {
  return paths.map((entry) => {
    if (resolvePath) {
      return resolvePath(entry);
    }
    return entry.startsWith("~") ? path.join(process.env.HOME ?? "", entry.slice(2)) : entry;
  });
}

export function resolveAssistantBotSidecarConfig(
  pluginConfig: Record<string, unknown> | undefined,
  resolvePath?: (input: string) => string,
): AssistantBotSidecarConfig {
  const raw = asRecord(pluginConfig);
  const watchRaw = asRecord(raw.watch);
  const cwd =
    typeof raw.cwd === "string" && raw.cwd.trim() ? raw.cwd.trim() : DEFAULT_TRADING_TOOLS_DIR;
  const command = typeof raw.command === "string" && raw.command.trim() ? raw.command.trim() : "uv";
  const args = asStringArray(raw.args) ?? [
    "run",
    "--project",
    DEFAULT_TRADING_TOOLS_DIR,
    "--package",
    "assistant-bot",
    "assistant-bot",
  ];
  const watchPaths = resolvePathList(
    asStringArray(watchRaw.paths) ?? DEFAULT_WATCH_PATHS,
    resolvePath,
  );

  return {
    enabled: raw.enabled !== false,
    cwd: resolvePath ? resolvePath(cwd) : cwd,
    command,
    args,
    env: normalizeEnv(raw.env),
    restartDelayMs: asNumber(raw.restartDelayMs, 1_000),
    shutdownGraceMs: asNumber(raw.shutdownGraceMs, 5_000),
    watch: {
      enabled: watchRaw.enabled !== false,
      debounceMs: asNumber(watchRaw.debounceMs, 750),
      paths: watchPaths,
    },
  };
}
