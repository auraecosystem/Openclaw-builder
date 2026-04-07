import type { OpenClawConfig } from "../config/config.js";
import { loadConfig } from "../config/config.js";
import { loadSessionStore, resolveStorePath, updateSessionStore } from "../config/sessions.js";
import { createSubsystemLogger } from "../logging/subsystem.js";
import { listAgentIds } from "./agent-scope.js";
import { listActiveEmbeddedRunSessionIds } from "./pi-embedded-runner/runs.js";

const log = createSubsystemLogger("embedded-run-orphan-recovery");

export async function reconcileOrphanedEmbeddedRunSessions(params?: {
  cfg?: OpenClawConfig;
  activeSessionIds?: Iterable<string>;
}): Promise<{ repaired: number; skipped: number; stores: number }> {
  const cfg = params?.cfg ?? loadConfig();
  const activeSessionIds = new Set(params?.activeSessionIds ?? listActiveEmbeddedRunSessionIds());
  const storePaths = new Set(
    listAgentIds(cfg).map((agentId) => resolveStorePath(cfg.session?.store, { agentId })),
  );
  let repaired = 0;
  let skipped = 0;

  for (const storePath of storePaths) {
    const store = loadSessionStore(storePath, { skipCache: true });
    if (Object.keys(store).length === 0) {
      continue;
    }
    await updateSessionStore(storePath, (currentStore) => {
      for (const [sessionKey, entry] of Object.entries(currentStore)) {
        if (!entry || entry.status !== "running" || !entry.sessionId) {
          continue;
        }
        if (activeSessionIds.has(entry.sessionId)) {
          skipped += 1;
          continue;
        }
        currentStore[sessionKey] = {
          ...entry,
          status: "killed",
          abortedLastRun: true,
          updatedAt: Date.now(),
        };
        repaired += 1;
      }
    });
  }

  if (repaired > 0 || skipped > 0) {
    log.info(
      `embedded run orphan reconciliation complete: repaired=${repaired} skipped=${skipped} stores=${storePaths.size}`,
    );
  }

  return { repaired, skipped, stores: storePaths.size };
}
