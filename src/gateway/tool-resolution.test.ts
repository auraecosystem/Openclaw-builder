import { beforeAll, describe, expect, it } from "vitest";
import type { OpenClawConfig } from "../config/types.openclaw.js";
import { resolveGatewayScopedTools } from "./tool-resolution.js";

describe("resolveGatewayScopedTools", () => {
  beforeAll(() => {
    resolveGatewayScopedTools({
      cfg: { tools: { profile: "minimal" } } as OpenClawConfig,
      sessionKey: "agent:main:telegram:group:-100123",
      messageProvider: "telegram",
      inboundEventKind: "room_event",
      surface: "loopback",
    });
  });

  it("force-allows the message tool for room-event loopback turns", () => {
    const result = resolveGatewayScopedTools({
      cfg: { tools: { profile: "minimal" } } as OpenClawConfig,
      sessionKey: "agent:main:telegram:group:-100123",
      messageProvider: "telegram",
      inboundEventKind: "room_event",
      surface: "loopback",
    });

    const messageTool = result.tools.find((tool) => tool.name === "message");
    expect(messageTool?.description).toContain(
      "visible replies to the current source conversation",
    );
  });

  it("keeps ordinary loopback turns under the configured profile", () => {
    const result = resolveGatewayScopedTools({
      cfg: { tools: { profile: "minimal" } } as OpenClawConfig,
      sessionKey: "agent:main:telegram:group:-100123",
      messageProvider: "telegram",
      inboundEventKind: "user_request",
      surface: "loopback",
    });

    expect(result.tools.some((tool) => tool.name === "message")).toBe(false);
  });

  it("marks HTTP-surface exec/process coding tools as ownerOnly (P1 fix for #63919)", () => {
    // Flagged by clawsweeper review: when gateway.tools.allow=["exec"] lifts
    // the default deny, the raw exec/process tools surfaced via the HTTP raw
    // coding factory must require owner semantics. A trusted-proxy caller
    // with operator.write but no operator.admin must not reach command exec.
    const result = resolveGatewayScopedTools({
      cfg: {
        tools: { profile: "full" },
        gateway: { tools: { allow: ["exec", "process"] } },
      } as OpenClawConfig,
      sessionKey: "main",
      surface: "http",
    });

    const exec = result.tools.find((tool) => tool.name === "exec");
    const proc = result.tools.find((tool) => tool.name === "process");
    // If exec/process surface at all (allowlist removed default deny), they
    // must be ownerOnly so applyOwnerOnlyToolPolicy filters them for non-owner
    // callers without requiring an entry in the global owner-only fallback set.
    if (exec) {
      expect(exec.ownerOnly).toBe(true);
    }
    if (proc) {
      expect(proc.ownerOnly).toBe(true);
    }
  });
});
