import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { CURRENT_SESSION_VERSION, SessionManager } from "@mariozechner/pi-coding-agent";
import { resolveSessionFilePath } from "../../config/sessions/paths.js";
import type { SessionEntry } from "../../config/sessions/types.js";

function normalizeComparablePath(value?: string): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }
  const trimmed = value.trim();
  return trimmed ? path.resolve(trimmed) : undefined;
}

function rewriteSessionHeaderCwd(sessionFile: string, cwd: string): void {
  const raw = fs.readFileSync(sessionFile, "utf-8");
  const newline = raw.includes("\r\n") ? "\r\n" : "\n";
  const lines = raw.split(/\r?\n/);
  if (lines.length === 0 || lines[0]?.trim().length === 0) {
    return;
  }
  const header = JSON.parse(lines[0]) as Record<string, unknown>;
  lines[0] = JSON.stringify({ ...header, cwd });
  fs.writeFileSync(sessionFile, lines.join(newline), {
    encoding: "utf-8",
    mode: 0o600,
  });
}

export function forkSessionFromParentRuntime(params: {
  parentEntry: SessionEntry;
  agentId: string;
  sessionsDir: string;
  targetCwd?: string;
}):
  | { status: "forked"; sessionId: string; sessionFile: string }
  | { status: "skipped"; reason: "cwd_mismatch"; parentCwd?: string; targetCwd?: string }
  | null {
  const parentSessionFile = resolveSessionFilePath(
    params.parentEntry.sessionId,
    params.parentEntry,
    { agentId: params.agentId, sessionsDir: params.sessionsDir },
  );
  if (!parentSessionFile || !fs.existsSync(parentSessionFile)) {
    return null;
  }
  try {
    const manager = SessionManager.open(parentSessionFile);
    const targetCwd = normalizeComparablePath(params.targetCwd);
    const parentCwd = normalizeComparablePath(
      (manager.getHeader() as { cwd?: unknown } | undefined)?.cwd as string | undefined,
    );
    if (targetCwd && parentCwd && parentCwd !== targetCwd) {
      return {
        status: "skipped",
        reason: "cwd_mismatch",
        parentCwd,
        targetCwd,
      };
    }
    const leafId = manager.getLeafId();
    if (leafId) {
      const sessionFile = manager.createBranchedSession(leafId) ?? manager.getSessionFile();
      const sessionId = manager.getSessionId();
      if (sessionFile && sessionId) {
        if (targetCwd) {
          rewriteSessionHeaderCwd(sessionFile, targetCwd);
        }
        return { status: "forked", sessionId, sessionFile };
      }
    }
    const sessionId = crypto.randomUUID();
    const timestamp = new Date().toISOString();
    const fileTimestamp = timestamp.replace(/[:.]/g, "-");
    const sessionFile = path.join(manager.getSessionDir(), `${fileTimestamp}_${sessionId}.jsonl`);
    const header = {
      type: "session",
      version: CURRENT_SESSION_VERSION,
      id: sessionId,
      timestamp,
      cwd: targetCwd ?? manager.getCwd(),
      parentSession: parentSessionFile,
    };
    fs.writeFileSync(sessionFile, `${JSON.stringify(header)}\n`, {
      encoding: "utf-8",
      mode: 0o600,
      flag: "wx",
    });
    return { status: "forked", sessionId, sessionFile };
  } catch {
    return null;
  }
}
