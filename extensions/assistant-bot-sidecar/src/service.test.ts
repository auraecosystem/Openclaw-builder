import { EventEmitter } from "node:events";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AssistantBotSidecarConfig } from "./config.js";

const { spawnMock, watchCloseMock, watchMock, existsSyncMock, statSyncMock } = vi.hoisted(() => {
  const watchClose = vi.fn();
  return {
    spawnMock: vi.fn(),
    watchCloseMock: watchClose,
    watchMock: vi.fn(() => ({ close: watchClose })),
    existsSyncMock: vi.fn(() => true),
    statSyncMock: vi.fn(() => ({ isDirectory: () => true })),
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
  },
}));

function createChild() {
  const child = new EventEmitter() as EventEmitter & {
    stdout: EventEmitter & { setEncoding: (encoding: string) => void };
    stderr: EventEmitter & { setEncoding: (encoding: string) => void };
    kill: ReturnType<typeof vi.fn>;
    killed: boolean;
  };
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
  cwd: "/tmp/trading-tools",
  command: "uv",
  args: ["run", "--project", "/tmp/trading-tools", "--package", "assistant-bot", "assistant-bot"],
  env: {},
  restartDelayMs: 10,
  shutdownGraceMs: 10,
  watch: {
    enabled: true,
    debounceMs: 5,
    paths: ["/tmp/trading-tools/apps/assistant_bot/src"],
  },
};

afterEach(() => {
  spawnMock.mockReset();
  watchMock.mockClear();
  watchCloseMock.mockClear();
  existsSyncMock.mockClear();
  statSyncMock.mockClear();
  existsSyncMock.mockReturnValue(true);
  statSyncMock.mockReturnValue({ isDirectory: () => true });
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

  it("starts and stops the assistant-bot child", async () => {
    const child = createChild();
    spawnMock.mockReturnValue(child);
    const { createAssistantBotSidecarService } = await import("./service.js");
    const logger = createLogger();
    const service = createAssistantBotSidecarService(baseConfig);

    await service.start({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
    expect(spawnMock).toHaveBeenCalledWith(
      "uv",
      ["run", "--project", "/tmp/trading-tools", "--package", "assistant-bot", "assistant-bot"],
      expect.objectContaining({ cwd: "/tmp/trading-tools" }),
    );
    expect(watchMock).toHaveBeenCalledTimes(1);

    await service.stop({ config: {} as never, logger, stateDir: "", workspaceDir: "" });
    expect(child.kill).toHaveBeenCalledWith("SIGTERM");
    expect(watchCloseMock).toHaveBeenCalled();
  });
});
