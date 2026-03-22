import path from "node:path";

export type ManagedWatchConfig = {
  enabled: boolean;
  debounceMs: number;
  paths: string[];
};

export type ManagedProcessConfig = {
  enabled: boolean;
  cwd: string;
  command: string;
  args: string[];
  envFiles: string[];
  env: Record<string, string>;
  restartDelayMs: number;
  shutdownGraceMs: number;
  watch: ManagedWatchConfig;
};

export type AssistantBotSidecarConfig = {
  enabled: boolean;
  startupGraceMs: number;
  daemon: ManagedProcessConfig;
  assistantBot: ManagedProcessConfig;
  sharedWatchPaths: string[];
};

const DEFAULT_TRADING_TOOLS_DIR = "/Users/ad/work/trading-tools";
const DEFAULT_STARTUP_GRACE_MS = 1_500;
const DEFAULT_DAEMON_WATCH_PATHS = [
  "/Users/ad/work/trading-tools/apps/trade_daemon/src",
  "/Users/ad/work/trading-tools/apps/trade_daemon/pyproject.toml",
  "/Users/ad/work/trading-tools/packages/trade_strategies/src",
];
const DEFAULT_ASSISTANT_BOT_WATCH_PATHS = [
  "/Users/ad/work/trading-tools/apps/assistant_bot/src",
  "/Users/ad/work/trading-tools/apps/assistant_bot/pyproject.toml",
];
const DEFAULT_SHARED_WATCH_PATHS = [
  "/Users/ad/work/trading-tools/packages/trade_core/src",
  "/Users/ad/work/trading-tools/packages/trade_journal/src",
];

type ManagedProcessDefaults = {
  cwd: string;
  command: string;
  args: string[];
  envFiles: string[];
  watchPaths: string[];
};

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

function resolveManagedProcessConfig(
  rawValue: unknown,
  defaults: ManagedProcessDefaults,
  resolvePath?: (input: string) => string,
): ManagedProcessConfig {
  const raw = asRecord(rawValue);
  const watchRaw = asRecord(raw.watch);
  const cwd = typeof raw.cwd === "string" && raw.cwd.trim() ? raw.cwd.trim() : defaults.cwd;
  const command =
    typeof raw.command === "string" && raw.command.trim() ? raw.command.trim() : defaults.command;
  const args = asStringArray(raw.args) ?? defaults.args;
  const envFiles = resolvePathList(asStringArray(raw.envFiles) ?? defaults.envFiles, resolvePath);
  const watchPaths = resolvePathList(
    asStringArray(watchRaw.paths) ?? defaults.watchPaths,
    resolvePath,
  );

  return {
    enabled: raw.enabled !== false,
    cwd: resolvePath ? resolvePath(cwd) : cwd,
    command,
    args,
    envFiles,
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

export function resolveAssistantBotSidecarConfig(
  pluginConfig: Record<string, unknown> | undefined,
  resolvePath?: (input: string) => string,
): AssistantBotSidecarConfig {
  const raw = asRecord(pluginConfig);

  return {
    enabled: raw.enabled !== false,
    startupGraceMs: asNumber(raw.startupGraceMs, DEFAULT_STARTUP_GRACE_MS),
    daemon: resolveManagedProcessConfig(
      raw.daemon,
      {
        cwd: DEFAULT_TRADING_TOOLS_DIR,
        command: "uv",
        args: [
          "run",
          "--project",
          DEFAULT_TRADING_TOOLS_DIR,
          "--package",
          "trade-daemon",
          "trade-daemon",
          "run",
        ],
        envFiles: [path.join(DEFAULT_TRADING_TOOLS_DIR, ".env")],
        watchPaths: DEFAULT_DAEMON_WATCH_PATHS,
      },
      resolvePath,
    ),
    assistantBot: resolveManagedProcessConfig(
      raw.assistantBot,
      {
        cwd: DEFAULT_TRADING_TOOLS_DIR,
        command: "uv",
        args: [
          "run",
          "--project",
          DEFAULT_TRADING_TOOLS_DIR,
          "--package",
          "assistant-bot",
          "assistant-bot",
        ],
        envFiles: [
          path.join(DEFAULT_TRADING_TOOLS_DIR, ".env"),
          path.join(DEFAULT_TRADING_TOOLS_DIR, "apps/assistant_bot/.env"),
        ],
        watchPaths: DEFAULT_ASSISTANT_BOT_WATCH_PATHS,
      },
      resolvePath,
    ),
    sharedWatchPaths: resolvePathList(
      asStringArray(raw.sharedWatchPaths) ?? DEFAULT_SHARED_WATCH_PATHS,
      resolvePath,
    ),
  };
}
