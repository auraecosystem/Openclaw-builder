import { EventEmitter } from "node:events";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AssistantBotSidecarConfig } from "./config.js";

type MockChild = EventEmitter & {
  stdout: EventEmitter & { setEncoding: (encoding: string) => void };
  stderr: EventEmitter & { setEncoding: (encoding: string) => void };
  kill: ReturnType<typeof vi.fn>;
  killed: boolean;
};

const { spawnMock, watchCloseMock, watchMock, existsSyncMock, statSyncMock, readFileSyncMock } =
  vi.hoisted(() => {
    const watchClose = vi.fn();
    return {
      spawnMock: vi.fn(),
      watchCloseMock: watchClose,
      watchMock: vi.fn(() => ({ close: watchClose })),
      existsSyncMock: vi.fn(() => true),
      statSyncMock: vi.fn(() => ({ isDirectory: () => true })),
      readFileSyncMock: vi.fn((path: string) => {
        if (path.endsWith("apps/assistant_bot/.env")) {
          return 'ASSISTANT_BOT_DSN="postgresql://postgres:postgres@127.0.0.1:15441/trading_tools?sslmode=disable"\n';
        }
        return [
          "TRADING_TOOLS_TIMESCALE_HOST=127.0.0.1",
          "TRADING_TOOLS_TIMESCALE_PORT=15441",
          "TRADING_TOOLS_TIMESCALE_DB=trading_tools",
          "TRADING_TOOLS_TIMESCALE_USER=postgres",
          "TRADING_TOOLS_TIMESCALE_PASSWORD=postgres",
          "TRADING_TOOLS_TIMESCALE_SSLMODE=disable",
        ].join("\n");
      }),
    };
  });

vi.mock("node:child_process", () => ({
  spawn: spawnMock,
}));

vi.mock("node:fs", () => ({
  default: {
    watch: watchMock,
    existsSync: existsSyncMock,
    statSync: statSyncMock,
    readFileSync: readFileSyncMock,
  },
}));

function createChild(): MockChild {
  const child = new EventEmitter() as MockChild;
  child.stdout = Object.assign(new EventEmitter(), { setEncoding: vi.fn() });
  child.stderr = Object.assign(new EventEmitter(), { setEncoding: vi.fn() });
  child.kill = vi.fn((signal?: string) => {
    child.killed = signal === "SIGKILL";
    child.emit("exit", 0, signal ?? null);
    return true;
  });
  child.killed = false;
  return child;
}

function createLogger() {
  return {
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  };
}

const baseConfig: AssistantBotSidecarConfig = {
  enabled: true,
  startupGraceMs: 10,
  daemon: {
    enabled: true,
    cwd: "/tmp/trading-tools",
    command: "uv",
    args: [
      "run",
      "--project",
      "/tmp/trading-tools",
      "--package",
      "trade-daemon",
      "trade-daemon",
      "run",
    ],
    envFiles: ["/tmp/trading-tools/.env"],
    env: {},
    restartDelayMs: 10,
    shutdownGraceMs: 10,
    watch: {
      enabled: true,
      debounceMs: 5,
      paths: ["/tmp/trading-tools/apps/trade_daemon/src"],
    },
  },
  assistantBot: {
    enabled: true,
    cwd: "/tmp/trading-tools",
    command: "uv",
    args: ["run", "--project", "/tmp/trading-tools", "--package", "assistant-bot", "assistant-bot"],
    envFiles: ["/tmp/trading-tools/.env", "/tmp/trading-tools/apps/assistant_bot/.env"],
    env: {},
    restartDelayMs: 10,
    shutdownGraceMs: 10,
    watch: {
      enabled: true,
      debounceMs: 5,
      paths: ["/tmp/trading-tools/apps/assistant_bot/src"],
    },
  },
  sharedWatchPaths: ["/tmp/trading-tools/packages/trade_core/src"],
};

async function startService(
  config: AssistantBotSidecarConfig = baseConfig,
  logger = createLogger(),
) {
  const { createAssistantBotSidecarService } = await import("./service.js");
  const service = createAssistantBotSidecarService(config);
  const startPromise = service.start({
    config: {} as never,
    logger,
    stateDir: "",
    workspaceDir: "",
  });
  await vi.advanceTimersByTimeAsync(config.startupGraceMs);
  await startPromise;
  return { service, logger };
}

beforeEach(() => {
  vi.useFakeTimers();
  existsSyncMock.mockReturnValue(true);
  statSyncMock.mockReturnValue({ isDirectory: () => true });
});

afterEach(() => {
  vi.useRealTimers();
  spawnMock.mockReset();
  watchMock.mockClear();
  watchCloseMock.mockClear();
  existsSyncMock.mockClear();
  statSyncMock.mockClear();
  readFileSyncMock.mockClear();
});

describe("assistant-bot sidecar service", () => {
  it("does nothing when disabled", async () => {
    const { createAssistantBotSidecarService } = await import("./service.js");
    const logger = createLogger();
    const service = createAssistantBotSidecarService({ ...baseConfig, enabled: false });

    await service.start({ config: {} as never, logger, stateDir: "", workspaceDir: "" });

    expect(spawnMock).not.toHaveBeenCalled();
    expect(watchMock).not.toHaveBeenCalled();
  });

  it("starts daemon then assistant-bot and stops in reverse order", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    spawnMock.mockReturnValueOnce(daemonChild).mockReturnValueOnce(assistantChild);

    const { service, logger } = await startService();

    expect(spawnMock).toHaveBeenNthCalledWith(
      1,
      "uv",
      [
        "run",
        "--project",
        "/tmp/trading-tools",
        "--package",
        "trade-daemon",
        "trade-daemon",
        "run",
      ],
      expect.objectContaining({
        cwd: "/tmp/trading-tools",
        env: expect.objectContaining({
          TRADE_DB_HOST: "127.0.0.1",
          TRADE_DB_PORT: "15441",
          TRADE_DB_NAME: "trading_tools",
          TRADE_DB_USER: "postgres",
          TRADE_DB_PASSWORD: "postgres",
          TRADE_DB_SSLMODE: "disable",
        }),
      }),
    );
    expect(spawnMock).toHaveBeenNthCalledWith(
      2,
      "uv",
      ["run", "--project", "/tmp/trading-tools", "--package", "assistant-bot", "assistant-bot"],
      expect.objectContaining({
        cwd: "/tmp/trading-tools",
        env: expect.objectContaining({
          ASSISTANT_BOT_DSN:
            "postgresql://postgres:postgres@127.0.0.1:15441/trading_tools?sslmode=disable",
          TRADE_DB_HOST: "127.0.0.1",
          TRADE_DB_PORT: "15441",
        }),
      }),
    );

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });

    expect(assistantChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(daemonChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(assistantChild.kill.mock.invocationCallOrder[0]).toBeLessThan(
      daemonChild.kill.mock.invocationCallOrder[0],
    );
  });

  it("restarts only assistant-bot on assistant-bot source changes", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    const assistantRestartChild = createChild();
    spawnMock
      .mockReturnValueOnce(daemonChild)
      .mockReturnValueOnce(assistantChild)
      .mockReturnValueOnce(assistantRestartChild);

    const { service, logger } = await startService();

    const assistantWatchCallback = watchMock.mock.calls[0]?.[2] as () => void;
    assistantWatchCallback();
    await vi.advanceTimersByTimeAsync(baseConfig.assistantBot.watch.debounceMs);

    expect(assistantChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(daemonChild.kill).not.toHaveBeenCalled();
    expect(spawnMock).toHaveBeenCalledTimes(3);

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("restarts daemon then assistant-bot on daemon source changes", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    const daemonRestartChild = createChild();
    const assistantRestartChild = createChild();
    spawnMock
      .mockReturnValueOnce(daemonChild)
      .mockReturnValueOnce(assistantChild)
      .mockReturnValueOnce(daemonRestartChild)
      .mockReturnValueOnce(assistantRestartChild);

    const { service, logger } = await startService();

    const daemonWatchCallback = watchMock.mock.calls[1]?.[2] as () => void;
    daemonWatchCallback();
    await vi.advanceTimersByTimeAsync(
      baseConfig.daemon.watch.debounceMs + baseConfig.startupGraceMs,
    );

    expect(assistantChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(daemonChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(assistantChild.kill.mock.invocationCallOrder[0]).toBeLessThan(
      daemonChild.kill.mock.invocationCallOrder[0],
    );
    expect(spawnMock).toHaveBeenCalledTimes(4);
    expect(spawnMock).toHaveBeenNthCalledWith(
      3,
      "uv",
      [
        "run",
        "--project",
        "/tmp/trading-tools",
        "--package",
        "trade-daemon",
        "trade-daemon",
        "run",
      ],
      expect.any(Object),
    );
    expect(spawnMock).toHaveBeenNthCalledWith(
      4,
      "uv",
      ["run", "--project", "/tmp/trading-tools", "--package", "assistant-bot", "assistant-bot"],
      expect.any(Object),
    );

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("restarts the full stack on shared source changes", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    const daemonRestartChild = createChild();
    const assistantRestartChild = createChild();
    spawnMock
      .mockReturnValueOnce(daemonChild)
      .mockReturnValueOnce(assistantChild)
      .mockReturnValueOnce(daemonRestartChild)
      .mockReturnValueOnce(assistantRestartChild);

    const { service, logger } = await startService();

    const sharedWatchCallback = watchMock.mock.calls[2]?.[2] as () => void;
    sharedWatchCallback();
    await vi.advanceTimersByTimeAsync(
      Math.max(baseConfig.daemon.watch.debounceMs, baseConfig.assistantBot.watch.debounceMs) +
        baseConfig.startupGraceMs,
    );

    expect(assistantChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(daemonChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(spawnMock).toHaveBeenCalledTimes(4);

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("restarts only assistant-bot after assistant-bot exits unexpectedly", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    const assistantRestartChild = createChild();
    spawnMock
      .mockReturnValueOnce(daemonChild)
      .mockReturnValueOnce(assistantChild)
      .mockReturnValueOnce(assistantRestartChild);

    const { service, logger } = await startService();

    assistantChild.emit("exit", 1, null);
    await vi.advanceTimersByTimeAsync(baseConfig.assistantBot.restartDelayMs);

    expect(daemonChild.kill).not.toHaveBeenCalled();
    expect(spawnMock).toHaveBeenCalledTimes(3);

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("restarts the full stack instead of assistant-bot when daemon is down", async () => {
    const daemonChild = createChild();
    const daemonRestartChild = createChild();
    const assistantRestartChild = createChild();
    spawnMock
      .mockReturnValueOnce(daemonChild)
      .mockReturnValueOnce(daemonRestartChild)
      .mockReturnValueOnce(assistantRestartChild);

    const logger = createLogger();
    const { createAssistantBotSidecarService } = await import("./service.js");
    const service = createAssistantBotSidecarService({
      ...baseConfig,
      daemon: {
        ...baseConfig.daemon,
        restartDelayMs: 50,
      },
    });
    const startPromise = service.start({
      config: {} as never,
      logger,
      stateDir: "",
      workspaceDir: "",
    });
    daemonChild.emit("exit", 1, null);
    await vi.advanceTimersByTimeAsync(baseConfig.startupGraceMs);
    await startPromise;

    const assistantWatchCallback = watchMock.mock.calls[0]?.[2] as () => void;
    assistantWatchCallback();
    await vi.advanceTimersByTimeAsync(
      baseConfig.assistantBot.watch.debounceMs + baseConfig.startupGraceMs,
    );

    expect(spawnMock).toHaveBeenCalledTimes(3);
    expect(logger.warn).toHaveBeenCalledWith(
      "assistant-bot-sidecar: assistant-bot restart requested while trade-daemon is down; restarting the full stack instead (source change under /tmp/trading-tools/apps/assistant_bot/src)",
    );

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("restarts daemon then assistant-bot after daemon exits unexpectedly", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    const daemonRestartChild = createChild();
    const assistantRestartChild = createChild();
    spawnMock
      .mockReturnValueOnce(daemonChild)
      .mockReturnValueOnce(assistantChild)
      .mockReturnValueOnce(daemonRestartChild)
      .mockReturnValueOnce(assistantRestartChild);

    const { service, logger } = await startService();

    daemonChild.emit("exit", 1, null);
    await vi.advanceTimersByTimeAsync(baseConfig.daemon.restartDelayMs + baseConfig.startupGraceMs);

    expect(assistantChild.kill).toHaveBeenCalledWith("SIGTERM");
    expect(spawnMock).toHaveBeenCalledTimes(4);

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("warns on missing watch paths and still starts enabled processes", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    existsSyncMock.mockReturnValueOnce(true).mockReturnValueOnce(false).mockReturnValueOnce(false);
    spawnMock.mockReturnValueOnce(daemonChild).mockReturnValueOnce(assistantChild);

    const logger = createLogger();
    const { service } = await startService(baseConfig, logger);

    expect(logger.warn).toHaveBeenCalledWith(
      "assistant-bot-sidecar: watch path missing: /tmp/trading-tools/apps/trade_daemon/src",
    );
    expect(logger.warn).toHaveBeenCalledWith(
      "assistant-bot-sidecar: watch path missing: /tmp/trading-tools/packages/trade_core/src",
    );
    expect(spawnMock).toHaveBeenCalledTimes(2);

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("warns on missing env files and still starts enabled processes", async () => {
    const daemonChild = createChild();
    const assistantChild = createChild();
    existsSyncMock.mockImplementation((input: string) => !input.endsWith(".env"));
    spawnMock.mockReturnValueOnce(daemonChild).mockReturnValueOnce(assistantChild);

    const logger = createLogger();
    const { service } = await startService(baseConfig, logger);

    expect(logger.warn).toHaveBeenCalledWith(
      "assistant-bot-sidecar: env file missing for trade-daemon: /tmp/trading-tools/.env",
    );
    expect(logger.warn).toHaveBeenCalledWith(
      "assistant-bot-sidecar: env file missing for assistant-bot: /tmp/trading-tools/.env",
    );
    expect(logger.warn).toHaveBeenCalledWith(
      "assistant-bot-sidecar: env file missing for assistant-bot: /tmp/trading-tools/apps/assistant_bot/.env",
    );
    expect(spawnMock).toHaveBeenCalledTimes(2);

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });

  it("can run assistant-bot without daemon when daemon is disabled", async () => {
    const assistantChild = createChild();
    spawnMock.mockReturnValueOnce(assistantChild);

    const logger = createLogger();
    const { service } = await startService(
      {
        ...baseConfig,
        daemon: {
          ...baseConfig.daemon,
          enabled: false,
        },
      },
      logger,
    );

    expect(spawnMock).toHaveBeenCalledTimes(1);
    expect(spawnMock).toHaveBeenCalledWith(
      "uv",
      ["run", "--project", "/tmp/trading-tools", "--package", "assistant-bot", "assistant-bot"],
      expect.objectContaining({ cwd: "/tmp/trading-tools" }),
    );
    expect(logger.warn).toHaveBeenCalledWith(
      "assistant-bot-sidecar: assistant-bot is enabled while trade-daemon is disabled; wake processing may be idle",
    );

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
  });
});
