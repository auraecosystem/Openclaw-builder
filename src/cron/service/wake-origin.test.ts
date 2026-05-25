import { describe, expect, it, vi } from "vitest";
import type { CronServiceState } from "./state.js";
import { wake } from "./timer.js";

// Minimal CronServiceState shim — `wake` only touches `state.deps` so the
// other state fields aren't relevant. Cast through `unknown` to avoid
// pulling in the full state factory just to exercise two callbacks.
function makeStateWithMocks(): {
  state: CronServiceState;
  enqueueSystemEvent: ReturnType<typeof vi.fn>;
  requestHeartbeat: ReturnType<typeof vi.fn>;
} {
  const enqueueSystemEvent = vi.fn();
  const requestHeartbeat = vi.fn();
  const state = {
    deps: { enqueueSystemEvent, requestHeartbeat },
  } as unknown as CronServiceState;
  return { state, enqueueSystemEvent, requestHeartbeat };
}

describe("cron service wake() origin capture", () => {
  it("forwards sessionKey + agentId to enqueueSystemEvent so the event lands on the originating session", () => {
    // Regression for openclaw/openclaw#46886 and #64556: prior to this
    // change the wake function called `enqueueSystemEvent(text)` with no
    // options, which routed every wake to the bound default (heartbeat /
    // main) regardless of which session the originating agent was in.
    const { state, enqueueSystemEvent, requestHeartbeat } = makeStateWithMocks();
    const result = wake(state, {
      mode: "now",
      text: "follow up on the report",
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      agentId: "main",
    });
    expect(result).toEqual({ ok: true });
    expect(enqueueSystemEvent).toHaveBeenCalledExactlyOnceWith("follow up on the report", {
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      agentId: "main",
    });
    expect(requestHeartbeat).toHaveBeenCalledExactlyOnceWith({
      source: "manual",
      intent: "immediate",
      reason: "wake",
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      agentId: "main",
    });
  });

  it("threads sessionKey + agentId into the targeted-immediate heartbeat for next-heartbeat+sessionKey too", () => {
    // Upstream's wake() collapses --mode now and --mode next-heartbeat into
    // the same targeted-immediate behavior when sessionKey is present — the
    // regularly-scheduled heartbeat fires for the agent's main session,
    // so a non-main wake needs an explicit targeted nudge to peek the
    // session's queue. agentId must thread through that nudge too.
    const { state, enqueueSystemEvent, requestHeartbeat } = makeStateWithMocks();
    const result = wake(state, {
      mode: "next-heartbeat",
      text: "check the queue",
      sessionKey: "agent:coding:discord:thread123",
      agentId: "coding",
    });
    expect(result).toEqual({ ok: true });
    expect(enqueueSystemEvent).toHaveBeenCalledExactlyOnceWith("check the queue", {
      sessionKey: "agent:coding:discord:thread123",
      agentId: "coding",
    });
    expect(requestHeartbeat).toHaveBeenCalledExactlyOnceWith({
      source: "manual",
      intent: "immediate",
      reason: "wake",
      sessionKey: "agent:coding:discord:thread123",
      agentId: "coding",
    });
  });

  it("preserves the pre-fix default-routing call shape when no sessionKey/agentId is provided", () => {
    // Backwards compatible: existing callers that omit origin must reach the
    // dep's default-sessionKey binding via the exact `enqueueSystemEvent(text,
    // undefined)` shape they expected pre-fix. Passing `{}` would route through
    // the same code path on most implementations, but the deps' binding is
    // free to distinguish "no opts" from "opts present but empty", so we
    // preserve the historical undefined.
    const { state, enqueueSystemEvent, requestHeartbeat } = makeStateWithMocks();
    const result = wake(state, { mode: "now", text: "no origin" });
    expect(result).toEqual({ ok: true });
    expect(enqueueSystemEvent).toHaveBeenCalledExactlyOnceWith("no origin", undefined);
    expect(requestHeartbeat).toHaveBeenCalledExactlyOnceWith({
      source: "manual",
      intent: "immediate",
      reason: "wake",
    });
  });

  it("does not nudge heartbeat for mode=next-heartbeat when no sessionKey was provided", () => {
    // Without sessionKey, the regularly-scheduled heartbeat will pick up the
    // queued event on its next tick — no targeted nudge is needed (and
    // would be wasteful). Pre-fix shape preserved.
    const { state, enqueueSystemEvent, requestHeartbeat } = makeStateWithMocks();
    wake(state, { mode: "next-heartbeat", text: "queued for next tick" });
    expect(enqueueSystemEvent).toHaveBeenCalledExactlyOnceWith("queued for next tick", undefined);
    expect(requestHeartbeat).not.toHaveBeenCalled();
  });

  it("drops whitespace-only sessionKey / agentId rather than routing to a meaningless lane", () => {
    // Defence-in-depth: gateway handler already trims, but the wake function
    // is also reachable directly by other in-process call sites. Empty /
    // whitespace fields must fall through to default routing, not route
    // the event to a session named " " (which would silently drop it).
    const { state, enqueueSystemEvent, requestHeartbeat } = makeStateWithMocks();
    wake(state, {
      mode: "now",
      text: "x",
      sessionKey: "   ",
      agentId: "\t",
    });
    expect(enqueueSystemEvent).toHaveBeenCalledExactlyOnceWith("x", undefined);
    expect(requestHeartbeat).toHaveBeenCalledExactlyOnceWith({
      source: "manual",
      intent: "immediate",
      reason: "wake",
    });
  });

  it("returns { ok: false } when text is empty so callers don't enqueue a meaningless event", () => {
    // Empty / whitespace text was rejected pre-fix; verify the new
    // signature didn't accidentally regress that guard while extending
    // the call shape.
    const { state, enqueueSystemEvent, requestHeartbeat } = makeStateWithMocks();
    expect(wake(state, { mode: "now", text: "   ", sessionKey: "agent:x" })).toEqual({ ok: false });
    expect(enqueueSystemEvent).not.toHaveBeenCalled();
    expect(requestHeartbeat).not.toHaveBeenCalled();
  });
});
