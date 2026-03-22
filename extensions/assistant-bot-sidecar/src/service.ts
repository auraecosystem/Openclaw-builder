import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import fs from "node:fs";
import type { OpenClawPluginService, PluginLogger } from "openclaw/plugin-sdk";
import type { AssistantBotSidecarConfig, ManagedProcessConfig } from "./config.js";

type ManagedProcessName = "daemon" | "assistantBot";
type RestartAction = "assistantBot" | "daemonAndAssistantBot";

type ManagedProcessState = {
  name: ManagedProcessName;
  label: string;
  config: ManagedProcessConfig;
  child: ChildProcessWithoutNullStreams | null;
  expectedExit: boolean;
  restartTimer: NodeJS.Timeout | null;
};

function describeCommand(command: string, args: string[]): string {
  return [command, ...args].join(" ");
}

function describeAction(action: RestartAction): string {
  return action === "assistantBot" ? "assistant-bot" : "trade-daemon -> assistant-bot";
}

function pipeChildOutput(
  processLabel: string,
  stream: NodeJS.ReadableStream,
  log: (message: string) => void,
  label: "stdout" | "stderr",
) {
  let buffer = "";
  stream.setEncoding("utf8");
  stream.on("data", (chunk: string) => {
    buffer += chunk;
    let newlineIndex = buffer.indexOf("\n");
    while (newlineIndex !== -1) {
      const line = buffer.slice(0, newlineIndex).trimEnd();
      buffer = buffer.slice(newlineIndex + 1);
      if (line) {
        log(`${processLabel} ${label}: ${line}`);
      }
      newlineIndex = buffer.indexOf("\n");
    }
  });
  stream.on("end", () => {
    const line = buffer.trimEnd();
    if (line) {
      log(`${processLabel} ${label}: ${line}`);
    }
  });
}

export function createAssistantBotSidecarService(
  config: AssistantBotSidecarConfig,
): OpenClawPluginService {
  const daemonState: ManagedProcessState = {
    name: "daemon",
    label: "trade-daemon",
    config: config.daemon,
    child: null,
    expectedExit: false,
    restartTimer: null,
  };
  const assistantBotState: ManagedProcessState = {
    name: "assistantBot",
    label: "assistant-bot",
    config: config.assistantBot,
    child: null,
    expectedExit: false,
    restartTimer: null,
  };

  let stopRequested = false;
  let watchTimer: NodeJS.Timeout | null = null;
  let watchAction: RestartAction | null = null;
  let watchReason = "";
  let startingStack: Promise<void> | null = null;
  let restartingStack: Promise<void> | null = null;
  let stoppingStack: Promise<void> | null = null;
  let watchers: fs.FSWatcher[] = [];

  const clearRestartTimer = (state: ManagedProcessState) => {
    if (state.restartTimer) {
      clearTimeout(state.restartTimer);
      state.restartTimer = null;
    }
  };

  const clearWatchTimer = () => {
    if (watchTimer) {
      clearTimeout(watchTimer);
      watchTimer = null;
    }
    watchAction = null;
    watchReason = "";
  };

  const closeWatchers = () => {
    for (const watcher of watchers) {
      watcher.close();
    }
    watchers = [];
  };

  const stopChild = async (state: ManagedProcessState) => {
    if (!state.child) {
      return;
    }
    const proc = state.child;
    state.expectedExit = true;
    state.child = null;
    await new Promise<void>((resolve) => {
      const forceKillTimer = setTimeout(() => {
        if (!proc.killed) {
          proc.kill("SIGKILL");
        }
      }, state.config.shutdownGraceMs);

      proc.once("exit", () => {
        clearTimeout(forceKillTimer);
        resolve();
      });

      proc.kill("SIGTERM");
    });
    state.expectedExit = false;
  };

  const scheduleManagedRestart = (
    state: ManagedProcessState,
    logger: PluginLogger,
    reason: string,
    action: () => Promise<void>,
  ) => {
    clearRestartTimer(state);
    logger.warn(
      `assistant-bot-sidecar: ${state.label} exited unexpectedly; restarting in ${state.config.restartDelayMs}ms (${reason})`,
    );
    state.restartTimer = setTimeout(() => {
      state.restartTimer = null;
      if (!stopRequested) {
        void action();
      }
    }, state.config.restartDelayMs);
  };

  const startChild = (state: ManagedProcessState, logger: PluginLogger) => {
    const commandLine = describeCommand(state.config.command, state.config.args);
    logger.info(`assistant-bot-sidecar: starting ${state.label}: ${commandLine}`);

    const child = spawn(state.config.command, state.config.args, {
      cwd: state.config.cwd,
      env: {
        ...process.env,
        ...state.config.env,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    state.child = child;
    state.expectedExit = false;

    pipeChildOutput(state.label, child.stdout, logger.info, "stdout");
    pipeChildOutput(state.label, child.stderr, logger.warn, "stderr");

    child.once("exit", (code, signal) => {
      const expected = stopRequested || state.expectedExit;
      if (state.child === child) {
        state.child = null;
      }
      state.expectedExit = false;
      if (expected) {
        logger.info(
          `assistant-bot-sidecar: ${state.label} stopped (code=${code ?? "null"} signal=${signal ?? "null"})`,
        );
        return;
      }

      if (state.name === "assistantBot") {
        scheduleManagedRestart(state, logger, `${state.label} crash`, async () =>
          restartAssistantBotOnly(logger, "assistant-bot exited unexpectedly"),
        );
        return;
      }

      scheduleManagedRestart(state, logger, `${state.label} crash`, async () =>
        restartDaemonAndAssistantBot(logger, "trade-daemon exited unexpectedly"),
      );
    });

    return child;
  };

  const waitForStartupGrace = async (
    state: ManagedProcessState,
    logger: PluginLogger,
  ): Promise<boolean> => {
    if (!state.child) {
      return false;
    }
    const child = state.child;
    if (config.startupGraceMs <= 0) {
      return state.child === child;
    }
    await new Promise<void>((resolve) => {
      setTimeout(resolve, config.startupGraceMs);
    });
    const stillRunning = state.child === child;
    if (stillRunning) {
      logger.info(
        `assistant-bot-sidecar: ${state.label} remained up for ${config.startupGraceMs}ms; continuing startup`,
      );
    }
    return stillRunning;
  };

  const stopBotThenDaemon = async (logger: PluginLogger, reason: string) => {
    logger.info(`assistant-bot-sidecar: stopping supervised stack (${reason})`);
    clearRestartTimer(assistantBotState);
    clearRestartTimer(daemonState);
    await stopChild(assistantBotState);
    await stopChild(daemonState);
  };

  const startDaemonThenBot = async (logger: PluginLogger, reason: string) => {
    logger.info(`assistant-bot-sidecar: starting supervised stack (${reason})`);
    clearRestartTimer(assistantBotState);
    clearRestartTimer(daemonState);

    if (daemonState.config.enabled) {
      startChild(daemonState, logger);
      const daemonReady = await waitForStartupGrace(daemonState, logger);
      if (!daemonReady) {
        logger.warn(
          "assistant-bot-sidecar: trade-daemon exited before startup grace completed; assistant-bot start deferred until daemon recovers",
        );
        return;
      }
    }

    if (assistantBotState.config.enabled) {
      startChild(assistantBotState, logger);
    }
  };

  const serializeOperation = async (
    current: Promise<void> | null,
    operation: () => Promise<void>,
  ): Promise<void> => {
    if (current) {
      await current;
    }
    return operation();
  };

  const restartAssistantBotOnly = async (logger: PluginLogger, reason: string) => {
    if (stopRequested || !assistantBotState.config.enabled) {
      return;
    }
    if (daemonState.config.enabled && !daemonState.child) {
      logger.warn(
        `assistant-bot-sidecar: assistant-bot restart requested while trade-daemon is down; restarting the full stack instead (${reason})`,
      );
      await restartDaemonAndAssistantBot(logger, reason);
      return;
    }
    const run = serializeOperation(restartingStack, async () => {
      logger.info(`assistant-bot-sidecar: restarting assistant-bot (${reason})`);
      clearRestartTimer(assistantBotState);
      await stopChild(assistantBotState);
      startChild(assistantBotState, logger);
    });
    restartingStack = run.finally(() => {
      if (restartingStack === run) {
        restartingStack = null;
      }
    });
    await run;
  };

  const restartDaemonAndAssistantBot = async (logger: PluginLogger, reason: string) => {
    if (stopRequested) {
      return;
    }
    const run = serializeOperation(restartingStack, async () => {
      logger.info(`assistant-bot-sidecar: restarting trade-daemon and assistant-bot (${reason})`);
      clearRestartTimer(assistantBotState);
      clearRestartTimer(daemonState);
      await stopBotThenDaemon(logger, reason);
      await startDaemonThenBot(logger, reason);
    });
    restartingStack = run.finally(() => {
      if (restartingStack === run) {
        restartingStack = null;
      }
    });
    await run;
  };

  const scheduleWatchRestart = (
    logger: PluginLogger,
    action: RestartAction,
    debounceMs: number,
    reason: string,
  ) => {
    if (stopRequested) {
      return;
    }
    const shouldReplace =
      !watchAction || watchAction === action || action === "daemonAndAssistantBot";
    if (!shouldReplace) {
      return;
    }
    clearWatchTimer();
    watchAction = action;
    watchReason = reason;
    watchTimer = setTimeout(() => {
      const pendingAction = watchAction;
      const pendingReason = watchReason;
      clearWatchTimer();
      if (pendingAction === "assistantBot") {
        void restartAssistantBotOnly(logger, pendingReason);
        return;
      }
      if (pendingAction === "daemonAndAssistantBot") {
        void restartDaemonAndAssistantBot(logger, pendingReason);
      }
    }, debounceMs);
    logger.info(
      `assistant-bot-sidecar: queued ${describeAction(action)} restart in ${debounceMs}ms (${reason})`,
    );
  };

  const startWatchGroup = (
    logger: PluginLogger,
    paths: string[],
    action: RestartAction,
    debounceMs: number,
  ) => {
    for (const watchPath of paths) {
      if (!fs.existsSync(watchPath)) {
        logger.warn(`assistant-bot-sidecar: watch path missing: ${watchPath}`);
        continue;
      }
      const stat = fs.statSync(watchPath);
      const watcher = fs.watch(
        watchPath,
        stat.isDirectory() ? { recursive: true } : undefined,
        () => {
          scheduleWatchRestart(logger, action, debounceMs, `source change under ${watchPath}`);
        },
      );
      watchers.push(watcher);
      logger.info(
        `assistant-bot-sidecar: watching ${watchPath} for ${describeAction(action)} restarts`,
      );
    }
  };

  const startWatchers = (logger: PluginLogger) => {
    if (assistantBotState.config.watch.enabled) {
      startWatchGroup(
        logger,
        assistantBotState.config.watch.paths,
        "assistantBot",
        assistantBotState.config.watch.debounceMs,
      );
    }
    if (daemonState.config.watch.enabled) {
      startWatchGroup(
        logger,
        daemonState.config.watch.paths,
        "daemonAndAssistantBot",
        daemonState.config.watch.debounceMs,
      );
    }
    const sharedDebounceMs = Math.max(
      daemonState.config.watch.debounceMs,
      assistantBotState.config.watch.debounceMs,
    );
    startWatchGroup(logger, config.sharedWatchPaths, "daemonAndAssistantBot", sharedDebounceMs);
  };

  return {
    id: "assistant-bot-sidecar",
    async start(ctx) {
      if (!config.enabled) {
        ctx.logger.info("assistant-bot-sidecar: disabled");
        return;
      }
      stopRequested = false;
      closeWatchers();
      clearWatchTimer();

      if (!daemonState.config.enabled && assistantBotState.config.enabled) {
        ctx.logger.warn(
          "assistant-bot-sidecar: assistant-bot is enabled while trade-daemon is disabled; wake processing may be idle",
        );
      }
      if (assistantBotState.config.enabled && !process.env.ASSISTANT_BOT_DISCORD_TOKEN) {
        ctx.logger.warn(
          "assistant-bot-sidecar: ASSISTANT_BOT_DISCORD_TOKEN is not set in the gateway environment; relying on assistant-bot local .env",
        );
      }

      startWatchers(ctx.logger);
      const run = serializeOperation(startingStack, async () => {
        await startDaemonThenBot(ctx.logger, "gateway start");
      });
      startingStack = run.finally(() => {
        if (startingStack === run) {
          startingStack = null;
        }
      });
      await run;
    },
    async stop(ctx) {
      stopRequested = true;
      closeWatchers();
      clearWatchTimer();
      const run = serializeOperation(stoppingStack, async () => {
        await stopBotThenDaemon(ctx.logger, "gateway stop");
      });
      stoppingStack = run.finally(() => {
        if (stoppingStack === run) {
          stoppingStack = null;
        }
      });
      await run;
      ctx.logger.info("assistant-bot-sidecar: stopped");
    },
  };
}
