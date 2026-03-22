import { describe, expect, it } from "vitest";
import { resolveAssistantBotSidecarConfig } from "./config.js";

describe("resolveAssistantBotSidecarConfig", () => {
  it("applies stack defaults", () => {
    const config = resolveAssistantBotSidecarConfig(undefined);

    expect(config.enabled).toBe(true);
    expect(config.startupGraceMs).toBe(1_500);

    expect(config.daemon.enabled).toBe(true);
    expect(config.daemon.cwd).toBe("/Users/ad/work/trading-tools");
    expect(config.daemon.command).toBe("uv");
    expect(config.daemon.args).toEqual([
      "run",
      "--project",
      "/Users/ad/work/trading-tools",
      "--package",
      "trade-daemon",
      "trade-daemon",
      "run",
    ]);
    expect(config.daemon.watch.paths).toContain(
      "/Users/ad/work/trading-tools/apps/trade_daemon/src",
    );

    expect(config.assistantBot.enabled).toBe(true);
    expect(config.assistantBot.cwd).toBe("/Users/ad/work/trading-tools");
    expect(config.assistantBot.command).toBe("uv");
    expect(config.assistantBot.args).toEqual([
      "run",
      "--project",
      "/Users/ad/work/trading-tools",
      "--package",
      "assistant-bot",
      "assistant-bot",
    ]);
    expect(config.assistantBot.watch.paths).toContain(
      "/Users/ad/work/trading-tools/apps/assistant_bot/src",
    );

    expect(config.sharedWatchPaths).toEqual([
      "/Users/ad/work/trading-tools/packages/trade_core/src",
      "/Users/ad/work/trading-tools/packages/trade_journal/src",
    ]);
  });

  it("honors explicit daemon and assistant-bot overrides", () => {
    const config = resolveAssistantBotSidecarConfig({
      enabled: false,
      startupGraceMs: 250,
      daemon: {
        enabled: false,
        cwd: "/tmp/trading-tools",
        command: "python3",
        args: ["-m", "trade_daemon", "run"],
        env: {
          TRADE_DAEMON_WORKER_ID: "test-worker",
        },
        restartDelayMs: 300,
        shutdownGraceMs: 900,
        watch: {
          enabled: false,
          debounceMs: 100,
          paths: ["/tmp/trade-daemon"],
        },
      },
      assistantBot: {
        enabled: true,
        cwd: "/tmp/trading-tools",
        command: "python3",
        args: ["-m", "assistant_bot"],
        env: {
          ASSISTANT_BOT_LOG_LEVEL: "DEBUG",
        },
        restartDelayMs: 450,
        shutdownGraceMs: 950,
        watch: {
          enabled: true,
          debounceMs: 125,
          paths: ["/tmp/assistant-bot"],
        },
      },
      sharedWatchPaths: ["/tmp/shared"],
    });

    expect(config.enabled).toBe(false);
    expect(config.startupGraceMs).toBe(250);

    expect(config.daemon.enabled).toBe(false);
    expect(config.daemon.cwd).toBe("/tmp/trading-tools");
    expect(config.daemon.command).toBe("python3");
    expect(config.daemon.args).toEqual(["-m", "trade_daemon", "run"]);
    expect(config.daemon.env.TRADE_DAEMON_WORKER_ID).toBe("test-worker");
    expect(config.daemon.restartDelayMs).toBe(300);
    expect(config.daemon.shutdownGraceMs).toBe(900);
    expect(config.daemon.watch.enabled).toBe(false);
    expect(config.daemon.watch.debounceMs).toBe(100);
    expect(config.daemon.watch.paths).toEqual(["/tmp/trade-daemon"]);

    expect(config.assistantBot.enabled).toBe(true);
    expect(config.assistantBot.cwd).toBe("/tmp/trading-tools");
    expect(config.assistantBot.command).toBe("python3");
    expect(config.assistantBot.args).toEqual(["-m", "assistant_bot"]);
    expect(config.assistantBot.env.ASSISTANT_BOT_LOG_LEVEL).toBe("DEBUG");
    expect(config.assistantBot.restartDelayMs).toBe(450);
    expect(config.assistantBot.shutdownGraceMs).toBe(950);
    expect(config.assistantBot.watch.enabled).toBe(true);
    expect(config.assistantBot.watch.debounceMs).toBe(125);
    expect(config.assistantBot.watch.paths).toEqual(["/tmp/assistant-bot"]);

    expect(config.sharedWatchPaths).toEqual(["/tmp/shared"]);
  });
});
