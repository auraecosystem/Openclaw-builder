import { describe, expect, it } from "vitest";
import type { OpenClawConfig } from "../config/config.js";
import {
  formatFastModeAutoLabel,
  formatFastModeAutoProgressText,
  formatFastModeStatusValue,
  resolveFastModeAutoSeconds,
  resolveFastModeForElapsed,
  resolveFastModeState,
} from "./fast-mode.js";

describe("resolveFastModeState", () => {
  it("prefers session overrides", () => {
    const state = resolveFastModeState({
      cfg: {} as OpenClawConfig,
      provider: "openai",
      model: "gpt-4o",
      sessionEntry: { fastMode: true },
    });

    expect(state.enabled).toBe(true);
    expect(state.mode).toBe(true);
    expect(state.source).toBe("session");
  });

  it("keeps auto as the persisted mode and starts enabled", () => {
    const state = resolveFastModeState({
      cfg: {} as OpenClawConfig,
      provider: "openai",
      model: "gpt-5.5",
      sessionEntry: { fastMode: "auto" },
    });

    expect(state.mode).toBe("auto");
    expect(state.enabled).toBe(true);
    expect(state.fastSeconds).toBe(60);
  });

  it("uses agent fastModeDefault when present", () => {
    const cfg = {
      agents: {
        list: [{ id: "alpha", fastModeDefault: true }],
      },
    } as OpenClawConfig;

    const state = resolveFastModeState({
      cfg,
      provider: "openai",
      model: "gpt-4o",
      agentId: "alpha",
    });

    expect(state.enabled).toBe(true);
    expect(state.source).toBe("agent");
  });

  it("falls back to model config when agent default is absent", () => {
    const cfg = {
      agents: {
        defaults: {
          models: {
            "openai/gpt-4o": { params: { fastMode: true } },
          },
        },
      },
    } as OpenClawConfig;

    const state = resolveFastModeState({
      cfg,
      provider: "openai",
      model: "gpt-4o",
    });

    expect(state.enabled).toBe(true);
    expect(state.source).toBe("config");
  });

  it("uses configured auto threshold from fast_seconds", () => {
    const cfg = {
      agents: {
        defaults: {
          models: {
            "openai/gpt-5.5": { params: { fastMode: "auto", fast_seconds: 45 } },
          },
        },
      },
    } as OpenClawConfig;

    const state = resolveFastModeState({
      cfg,
      provider: "openai",
      model: "gpt-5.5",
    });

    expect(state.mode).toBe("auto");
    expect(state.enabled).toBe(true);
    expect(state.fastSeconds).toBe(45);
    expect(state.source).toBe("config");
  });

  it("formats auto mode with the active threshold", () => {
    const cfg = {
      agents: {
        defaults: {
          models: {
            "openai-codex/gpt-5.5": { params: { fastMode: "auto", fast_seconds: 2 } },
          },
        },
      },
    } as OpenClawConfig;
    const fastSeconds = resolveFastModeAutoSeconds({
      cfg,
      provider: "openai-codex",
      model: "gpt-5.5",
    });

    expect(fastSeconds).toBe(2);
    expect(formatFastModeAutoLabel(fastSeconds)).toBe("auto (2 sec)");
    expect(formatFastModeStatusValue({ mode: "auto", fastSeconds })).toBe("auto (2 sec)");
    expect(formatFastModeStatusValue({ mode: true, fastSeconds })).toBe("on");
  });

  it("uses model config when the runtime passes a provider-qualified model ref", () => {
    const cfg = {
      agents: {
        defaults: {
          models: {
            "openai/gpt-5.5": { params: { fastMode: true } },
          },
        },
      },
    } as OpenClawConfig;

    const state = resolveFastModeState({
      cfg,
      provider: "openai",
      model: "openai/gpt-5.5",
    });

    expect(state.enabled).toBe(true);
    expect(state.source).toBe("config");
  });

  it("uses canonical provider/model config for slash-containing model ids", () => {
    const cfg = {
      agents: {
        defaults: {
          models: {
            "openrouter/anthropic/claude-sonnet-4-6": { params: { fastMode: true } },
          },
        },
      },
    } as OpenClawConfig;

    const state = resolveFastModeState({
      cfg,
      provider: "openrouter",
      model: "anthropic/claude-sonnet-4-6",
    });

    expect(state.enabled).toBe(true);
    expect(state.source).toBe("config");
  });

  it("does not use another provider's slash-containing model config", () => {
    const cfg = {
      agents: {
        defaults: {
          models: {
            "anthropic/claude-sonnet-4-6": { params: { fastMode: true } },
          },
        },
      },
    } as OpenClawConfig;

    const state = resolveFastModeState({
      cfg,
      provider: "openrouter",
      model: "anthropic/claude-sonnet-4-6",
    });

    expect(state.enabled).toBe(false);
    expect(state.source).toBe("default");
  });

  it("defaults to off when unset", () => {
    const state = resolveFastModeState({
      cfg: {} as OpenClawConfig,
      provider: "openai",
      model: "gpt-4o",
    });

    expect(state.enabled).toBe(false);
    expect(state.source).toBe("default");
  });
});

describe("resolveFastModeForElapsed", () => {
  it("keeps auto on through the exact threshold", () => {
    expect(
      resolveFastModeForElapsed({
        mode: "auto",
        fastSeconds: 60,
        startedAtMs: 1_000,
        nowMs: 61_000,
      }),
    ).toMatchObject({
      mode: "auto",
      enabled: true,
      elapsedSeconds: 60,
      fastSeconds: 60,
    });
  });

  it("turns auto off after the threshold", () => {
    expect(
      resolveFastModeForElapsed({
        mode: "auto",
        fastSeconds: 60,
        startedAtMs: 1_000,
        nowMs: 76_000,
      }),
    ).toMatchObject({
      mode: "auto",
      enabled: false,
      elapsedSeconds: 75,
      fastSeconds: 60,
    });
  });

  it("formats auto transition progress", () => {
    expect(
      formatFastModeAutoProgressText({
        enabled: false,
        elapsedSeconds: 75,
        fastSeconds: 60,
      }),
    ).toBe("💨Fast: auto-off(75s>60s)");
    expect(
      formatFastModeAutoProgressText({
        enabled: true,
        elapsedSeconds: 0,
        fastSeconds: 60,
      }),
    ).toBe("💨Fast: auto-on");
  });
});
