import { describe, expect, it } from "vitest";
import type { SessionEntry } from "../../config/sessions.js";
import { parseInlineDirectives } from "./directive-handling.parse.js";
import { type ReplyExecOverrides, resolveReplyExecOverrides } from "./get-reply-exec-overrides.js";

const AGENT_EXEC_DEFAULTS = {
  host: "node",
  mode: "ask",
  security: "allowlist",
  ask: "always",
  node: "worker-alpha",
} as const satisfies ReplyExecOverrides;

function createSessionEntry(overrides?: Partial<SessionEntry>): SessionEntry {
  return {
    sessionId: "main",
    updatedAt: Date.now(),
    ...overrides,
  };
}

describe("reply exec overrides", () => {
  it("uses per-agent exec defaults when session and message are unset", () => {
    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("run a command"),
        sessionEntry: createSessionEntry(),
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toEqual(AGENT_EXEC_DEFAULTS);
  });

  it("prefers inline exec directives, then persisted session overrides, then agent defaults", () => {
    const sessionEntry = createSessionEntry({
      execHost: "gateway",
      execMode: "auto",
      execSecurity: "deny",
    });

    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("/exec host=auto mode=full security=full"),
        sessionEntry,
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toEqual({
      host: "auto",
      mode: "full",
      security: undefined,
      ask: undefined,
      node: "worker-alpha",
    });

    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("run a command"),
        sessionEntry,
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toEqual({
      ...AGENT_EXEC_DEFAULTS,
      host: "gateway",
      mode: undefined,
      security: "deny",
    });
  });

  it("uses persisted session exec fields for later turns", () => {
    const sessionEntry = createSessionEntry({
      execHost: "gateway",
      execSecurity: "full",
      execAsk: "always",
    });

    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("run a command"),
        sessionEntry,
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toEqual({
      host: "gateway",
      mode: undefined,
      security: "full",
      ask: "always",
      node: "worker-alpha",
    });
  });

  it("does not carry lower-scope mode through a narrower legacy policy override", () => {
    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("/exec security=deny"),
        sessionEntry: createSessionEntry({
          execMode: "auto",
        }),
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toMatchObject({
      mode: undefined,
      security: "deny",
    });

    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("run a command"),
        sessionEntry: createSessionEntry({
          execAsk: "always",
        }),
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toMatchObject({
      mode: undefined,
      ask: "always",
    });

    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("run a command"),
        sessionEntry: createSessionEntry({
          execMode: "auto",
          execSecurity: "full",
        }),
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toMatchObject({
      mode: undefined,
      security: "full",
    });
  });

  it("drops legacy policy fields when an inline exec mode is present", () => {
    expect(
      resolveReplyExecOverrides({
        directives: parseInlineDirectives("/exec mode=full security=deny ask=always"),
        sessionEntry: createSessionEntry({
          execSecurity: "deny",
          execAsk: "always",
        }),
        agentExecDefaults: AGENT_EXEC_DEFAULTS,
      }),
    ).toEqual({
      host: "node",
      mode: "full",
      security: undefined,
      ask: undefined,
      node: "worker-alpha",
    });
  });
});
