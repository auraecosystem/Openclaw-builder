import { describe, expect, it, vi } from "vitest";
import type { DeliveryContext } from "../../utils/delivery-context.types.js";
import type { CronJob } from "../types.js";
import type { CronServiceState } from "./state.js";
import { executeJobCore, wake } from "./timer.js";

// These cover the "capture origin delivery context, carry it to the wake
// event" fix: a sessionTarget:"main" wake-now job (and the wake() tool path)
// must thread the bound channel thread/topic (e.g. Telegram topic 4052) onto
// the enqueued system event's deliveryContext so the delivered heartbeat
// routes back into the originating thread instead of the chat root.
//
// The channel-correct threadId is sourced via the resolveOriginDeliveryContext
// dep (implemented in server-cron from the session store), NOT by splitting the
// composite session-key thread suffix. The tests mock that dep so they exercise
// only timer.ts's carry behavior.

const TOPIC_DELIVERY_CONTEXT: DeliveryContext = {
  channel: "telegram",
  to: "telegram:8661849123:topic:4052",
  accountId: "default",
  threadId: "4052",
};

function makeStateWithMocks(
  resolveOriginDeliveryContext?: (params: {
    sessionKey?: string;
    agentId?: string;
  }) => DeliveryContext | undefined,
): {
  state: CronServiceState;
  enqueueSystemEvent: ReturnType<typeof vi.fn>;
  requestHeartbeat: ReturnType<typeof vi.fn>;
  resolveOriginDeliveryContext: ReturnType<typeof vi.fn>;
} {
  const enqueueSystemEvent = vi.fn();
  const requestHeartbeat = vi.fn();
  const resolveOrigin = vi.fn(resolveOriginDeliveryContext ?? (() => undefined));
  const state = {
    deps: {
      enqueueSystemEvent,
      requestHeartbeat,
      resolveOriginDeliveryContext: resolveOrigin,
      // runHeartbeatOnce intentionally omitted so a main wake-now job stops
      // after enqueue (the heartbeat dispatch is out of scope for these tests).
    },
  } as unknown as CronServiceState;
  return { state, enqueueSystemEvent, requestHeartbeat, resolveOriginDeliveryContext: resolveOrigin };
}

function makeMainSystemEventJob(overrides?: Partial<CronJob>): CronJob {
  return {
    id: "job-topic-4052",
    agentId: "main",
    sessionKey: "agent:main:telegram:8661849123:topic:4052",
    name: "topic reminder",
    enabled: true,
    createdAtMs: 0,
    updatedAtMs: 0,
    schedule: { kind: "every", everyMs: 60_000 },
    sessionTarget: "main",
    wakeMode: "next-heartbeat",
    payload: { kind: "systemEvent", text: "follow up on the report" },
    state: {},
    ...overrides,
  } as CronJob;
}

describe("cron main job origin delivery-context carry", () => {
  it("attaches the bound thread's deliveryContext to the enqueued system event", async () => {
    const { state, enqueueSystemEvent, resolveOriginDeliveryContext } = makeStateWithMocks(
      () => TOPIC_DELIVERY_CONTEXT,
    );
    const job = makeMainSystemEventJob();

    await executeJobCore(state, job);

    expect(resolveOriginDeliveryContext).toHaveBeenCalledWith({
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      agentId: "main",
    });
    expect(enqueueSystemEvent).toHaveBeenCalledTimes(1);
    expect(enqueueSystemEvent).toHaveBeenCalledWith("follow up on the report", {
      agentId: "main",
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      contextKey: "cron:job-topic-4052",
      deliveryContext: TOPIC_DELIVERY_CONTEXT,
    });
  });

  it("omits deliveryContext when no origin context resolves (unchanged default routing)", async () => {
    const { state, enqueueSystemEvent } = makeStateWithMocks(() => undefined);
    const job = makeMainSystemEventJob();

    await executeJobCore(state, job);

    expect(enqueueSystemEvent).toHaveBeenCalledTimes(1);
    expect(enqueueSystemEvent).toHaveBeenCalledWith("follow up on the report", {
      agentId: "main",
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      contextKey: "cron:job-topic-4052",
    });
    const [, options] = enqueueSystemEvent.mock.calls[0] as [string, Record<string, unknown>];
    expect(options).not.toHaveProperty("deliveryContext");
  });

  it("works when no resolveOriginDeliveryContext dep is wired (legacy deps)", async () => {
    const { state, enqueueSystemEvent } = makeStateWithMocks();
    // Drop the dep entirely to mirror a deployment whose adapter predates the fix.
    (state.deps as { resolveOriginDeliveryContext?: unknown }).resolveOriginDeliveryContext =
      undefined;
    const job = makeMainSystemEventJob();

    await executeJobCore(state, job);

    expect(enqueueSystemEvent).toHaveBeenCalledWith("follow up on the report", {
      agentId: "main",
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      contextKey: "cron:job-topic-4052",
    });
  });
});

describe("cron wake() origin delivery-context carry", () => {
  it("threads the resolved deliveryContext onto a sessionKey-targeted wake", () => {
    const { state, enqueueSystemEvent, resolveOriginDeliveryContext } = makeStateWithMocks(
      () => TOPIC_DELIVERY_CONTEXT,
    );

    const result = wake(state, {
      mode: "now",
      text: "check the queue",
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      agentId: "main",
    });

    expect(result).toEqual({ ok: true });
    expect(resolveOriginDeliveryContext).toHaveBeenCalledWith({
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      agentId: "main",
    });
    expect(enqueueSystemEvent).toHaveBeenCalledExactlyOnceWith("check the queue", {
      sessionKey: "agent:main:telegram:8661849123:topic:4052",
      agentId: "main",
      deliveryContext: TOPIC_DELIVERY_CONTEXT,
    });
  });

  it("keeps the no-origin call shape (enqueueSystemEvent(text, undefined)) when untargeted", () => {
    const { state, enqueueSystemEvent, resolveOriginDeliveryContext } = makeStateWithMocks(
      () => TOPIC_DELIVERY_CONTEXT,
    );

    wake(state, { mode: "now", text: "no origin" });

    // Untargeted wakes must not even consult the resolver, preserving the exact
    // pre-fix default-sessionKey binding behavior.
    expect(resolveOriginDeliveryContext).not.toHaveBeenCalled();
    expect(enqueueSystemEvent).toHaveBeenCalledExactlyOnceWith("no origin", undefined);
  });
});
