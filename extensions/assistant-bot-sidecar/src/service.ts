import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import fs from "node:fs";
import type { OpenClawPluginService, PluginLogger } from "openclaw/plugin-sdk";
import type { AssistantBotSidecarConfig } from "./config.js";

function describeCommand(command: string, args: string[]): string {
  return [command, ...args].join(" ");
}

function pipeChildOutput(
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
        log(`assistant-bot ${label}: ${line}`);
      }
      newlineIndex = buffer.indexOf("\n");
    }
  });
  stream.on("end", () => {
    const line = buffer.trimEnd();
    if (line) {
      log(`assistant-bot ${label}: ${line}`);
    }
  });
}

export function createAssistantBotSidecarService(
  config: AssistantBotSidecarConfig,
): OpenClawPluginService {
  let child: ChildProcessWithoutNullStreams | null = null;
  let stopRequested = false;
  let restartTimer: NodeJS.Timeout | null = null;
  let watchDebounceTimer: NodeJS.Timeout | null = null;
  let watchers: fs.FSWatcher[] = [];

  const clearRestartTimer = () => {
    if (restartTimer) {
      clearTimeout(restartTimer);
      restartTimer = null;
    }
  };

  const clearWatchDebounceTimer = () => {
    if (watchDebounceTimer) {
      clearTimeout(watchDebounceTimer);
      watchDebounceTimer = null;
    }
  };

  const closeWatchers = () => {
    for (const watcher of watchers) {
      watcher.close();
    }
    watchers = [];
  };

  const stopChild = async () => {
    if (!child) {
      return;
    }
    const proc = child;
    child = null;
    proc.kill("SIGTERM");

    await new Promise<void>((resolve) => {
      const forceKillTimer = setTimeout(() => {
        if (!proc.killed) {
          proc.kill("SIGKILL");
        }
      }, config.shutdownGraceMs);

      proc.once("exit", () => {
        clearTimeout(forceKillTimer);
        resolve();
      });
    });
  };

  const startChild = (logger: PluginLogger) => {
    const commandLine = describeCommand(config.command, config.args);
    logger.info(`assistant-bot-sidecar: starting ${commandLine}`);

    child = spawn(config.command, config.args, {
      cwd: config.cwd,
      env: {
        ...process.env,
        ...config.env,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    pipeChildOutput(child.stdout, logger.info, "stdout");
    pipeChildOutput(child.stderr, logger.warn, "stderr");

    child.once("exit", (code, signal) => {
      const expected = stopRequested;
      child = null;
      if (expected) {
        logger.info(
          `assistant-bot-sidecar: stopped (code=${code ?? "null"} signal=${signal ?? "null"})`,
        );
        return;
      }

      logger.warn(
        `assistant-bot-sidecar: exited unexpectedly (code=${code ?? "null"} signal=${signal ?? "null"}); restarting in ${config.restartDelayMs}ms`,
      );
      clearRestartTimer();
      restartTimer = setTimeout(() => {
        restartTimer = null;
        if (!stopRequested) {
          startChild(logger);
        }
      }, config.restartDelayMs);
    });
  };

  const restartChild = async (logger: PluginLogger, reason: string) => {
    if (stopRequested) {
      return;
    }
    logger.info(`assistant-bot-sidecar: restarting (${reason})`);
    clearRestartTimer();
    await stopChild();
    if (!stopRequested) {
      startChild(logger);
    }
  };

  const startWatchers = (logger: PluginLogger) => {
    if (!config.watch.enabled) {
      return;
    }

    for (const watchPath of config.watch.paths) {
      if (!fs.existsSync(watchPath)) {
        logger.warn(`assistant-bot-sidecar: watch path missing: ${watchPath}`);
        continue;
      }
      const stat = fs.statSync(watchPath);
      const watcher = fs.watch(
        watchPath,
        stat.isDirectory() ? { recursive: true } : undefined,
        () => {
          if (stopRequested) {
            return;
          }
          clearWatchDebounceTimer();
          watchDebounceTimer = setTimeout(() => {
            watchDebounceTimer = null;
            void restartChild(logger, `source change under ${watchPath}`);
          }, config.watch.debounceMs);
        },
      );
      watchers.push(watcher);
      logger.info(`assistant-bot-sidecar: watching ${watchPath}`);
    }
  };

  return {
    id: "assistant-bot-sidecar",
    async start(ctx) {
      if (!config.enabled) {
        ctx.logger.info("assistant-bot-sidecar: disabled");
        return;
      }
      stopRequested = false;
      const missingEnv = !process.env.ASSISTANT_BOT_DISCORD_TOKEN;
      if (missingEnv) {
        ctx.logger.warn(
          "assistant-bot-sidecar: ASSISTANT_BOT_DISCORD_TOKEN is not set; startup may fail until the environment is configured",
        );
      }
      startChild(ctx.logger);
      startWatchers(ctx.logger);
    },
    async stop(ctx) {
      stopRequested = true;
      clearRestartTimer();
      clearWatchDebounceTimer();
      closeWatchers();
      await stopChild();
      ctx.logger.info("assistant-bot-sidecar: stopped");
    },
  };
}
