import { describe, expect, it } from "vitest";
import { resolveAssistantBotSidecarConfig } from "./config.js";

describe("resolveAssistantBotSidecarConfig", () => {
  it("applies local trading defaults", () => {
    const config = resolveAssistantBotSidecarConfig(undefined);

    expect(config.enabled).toBe(true);
    expect(config.cwd).toBe("/Users/ad/work/trading-tools");
    expect(config.command).toBe("uv");
    expect(config.args).toEqual([
      "run",
      "--project",
      "/Users/ad/work/trading-tools",
      "--package",
      "assistant-bot",
      "assistant-bot",
    ]);
    expect(config.watch.enabled).toBe(true);
    expect(config.watch.paths).toContain("/Users/ad/work/trading-tools/apps/assistant_bot/src");
  });

  it("honors explicit overrides", () => {
    const config = resolveAssistantBotSidecarConfig({
      enabled: false,
      cwd: "/tmp/trading-tools",
      command: "python3",
      args: ["-m", "assistant_bot"],
      env: {
        ASSISTANT_BOT_LOG_LEVEL: "DEBUG",
      },
      restartDelayMs: 250,
      shutdownGraceMs: 900,
      watch: {
        enabled: false,
        debounceMs: 100,
        paths: ["/tmp/assistant-bot"],
      },
    });

    expect(config.enabled).toBe(false);
    expect(config.cwd).toBe("/tmp/trading-tools");
    expect(config.command).toBe("python3");
    expect(config.args).toEqual(["-m", "assistant_bot"]);
    expect(config.env.ASSISTANT_BOT_LOG_LEVEL).toBe("DEBUG");
    expect(config.restartDelayMs).toBe(250);
    expect(config.shutdownGraceMs).toBe(900);
    expect(config.watch.enabled).toBe(false);
    expect(config.watch.debounceMs).toBe(100);
    expect(config.watch.paths).toEqual(["/tmp/assistant-bot"]);
  });
});
