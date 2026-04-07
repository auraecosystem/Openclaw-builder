import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { loadSessionStore, resolveStorePath, updateSessionStore } from "../config/sessions.js";
import { reconcileOrphanedEmbeddedRunSessions } from "./embedded-run-orphan-recovery.js";

describe("reconcileOrphanedEmbeddedRunSessions", () => {
  const tempPaths: string[] = [];

  afterEach(async () => {
    while (tempPaths.length > 0) {
      const target = tempPaths.pop();
      if (target) {
        await fs.rm(target, { recursive: true, force: true });
      }
    }
  });

  it("marks orphaned running sessions killed and aborted", async () => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), "openclaw-embedded-run-orphans-"));
    tempPaths.push(root);
    const cfg = {
      session: {
        store: path.join(root, "{agentId}", "sessions", "sessions.json"),
      },
      agents: {
        list: [{ id: "main", default: true }],
      },
    };
    const storePath = resolveStorePath(cfg.session.store, { agentId: "main" });

    await updateSessionStore(storePath, (store) => {
      store["agent:main:discord:channel:1"] = {
        sessionId: "session-orphaned",
        updatedAt: 1,
        status: "running",
        abortedLastRun: false,
      };
    });

    const result = await reconcileOrphanedEmbeddedRunSessions({
      cfg: cfg as never,
      activeSessionIds: [],
    });

    expect(result).toMatchObject({ repaired: 1, skipped: 0, stores: 1 });
    expect(
      loadSessionStore(storePath, { skipCache: true })["agent:main:discord:channel:1"],
    ).toMatchObject({
      status: "killed",
      abortedLastRun: true,
    });
  });

  it("keeps actively tracked running sessions unchanged", async () => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), "openclaw-embedded-run-active-"));
    tempPaths.push(root);
    const cfg = {
      session: {
        store: path.join(root, "{agentId}", "sessions", "sessions.json"),
      },
      agents: {
        list: [{ id: "main", default: true }],
      },
    };
    const storePath = resolveStorePath(cfg.session.store, { agentId: "main" });

    await updateSessionStore(storePath, (store) => {
      store["agent:main:discord:channel:2"] = {
        sessionId: "session-active",
        updatedAt: 1,
        status: "running",
        abortedLastRun: false,
      };
    });

    const result = await reconcileOrphanedEmbeddedRunSessions({
      cfg: cfg as never,
      activeSessionIds: ["session-active"],
    });

    expect(result).toMatchObject({ repaired: 0, skipped: 1, stores: 1 });
    expect(
      loadSessionStore(storePath, { skipCache: true })["agent:main:discord:channel:2"],
    ).toMatchObject({
      status: "running",
      abortedLastRun: false,
    });
  });
});
