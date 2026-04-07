export const POST_TOOL_CONTINUATION_TIMEOUT_MS = 20_000;
export const POST_TOOL_CONTINUATION_TIMEOUT_ERROR_TEXT =
  "Tool continuation timeout (20s): no follow-up after completed tool result";

export type ToolContinuationWatchdogAgentEvent = {
  stream: string;
  data: Record<string, unknown>;
};

type TerminalToolResultSnapshot = {
  toolCallId: string;
  toolName: string;
  endedAt: number;
  status?: string;
};

type ToolContinuationWatchdogParams = {
  runId: string;
  sessionId: string;
  sessionKey?: string;
  isProbeSession?: boolean;
  timeoutMs?: number;
  now?: () => number;
  setTimer?: typeof setTimeout;
  clearTimer?: typeof clearTimeout;
  emitFailureNote: () => Promise<void>;
  onTimeout: (error: Error) => void;
  logDebug?: (message: string) => void;
  logError?: (message: string) => void;
};

function readToolResultStatusFromEvent(
  evt: ToolContinuationWatchdogAgentEvent,
): string | undefined {
  if (evt.stream !== "tool" || evt.data.phase !== "result") {
    return undefined;
  }
  const result = evt.data.result;
  if (!result || typeof result !== "object" || Array.isArray(result)) {
    return undefined;
  }
  const details = (result as { details?: unknown }).details;
  if (!details || typeof details !== "object" || Array.isArray(details)) {
    return undefined;
  }
  const status = (details as { status?: unknown }).status;
  return typeof status === "string" ? status.trim().toLowerCase() : undefined;
}

function readTerminalToolResultFromEvent(
  evt: ToolContinuationWatchdogAgentEvent,
  now: () => number,
): TerminalToolResultSnapshot | undefined {
  if (evt.stream !== "tool" || evt.data.phase !== "result") {
    return undefined;
  }
  const toolCallId = typeof evt.data.toolCallId === "string" ? evt.data.toolCallId.trim() : "";
  const toolName = typeof evt.data.name === "string" ? evt.data.name.trim() : "";
  if (!toolCallId || !toolName) {
    return undefined;
  }
  const status = readToolResultStatusFromEvent(evt);
  if (status === "running") {
    return undefined;
  }
  return {
    toolCallId,
    toolName,
    endedAt: now(),
    ...(status ? { status } : {}),
  };
}

function isContinuationProgressEvent(evt: ToolContinuationWatchdogAgentEvent): boolean {
  if (evt.stream === "assistant") {
    return true;
  }
  if (evt.stream === "compaction") {
    return true;
  }
  if (evt.stream === "lifecycle") {
    const phase = typeof evt.data.phase === "string" ? evt.data.phase : "";
    return phase === "start" || phase === "end" || phase === "error";
  }
  if (evt.stream === "tool") {
    return evt.data.phase === "start";
  }
  return false;
}

function describeContinuationPhase(evt: ToolContinuationWatchdogAgentEvent): string {
  const phase = evt.data.phase;
  return typeof phase === "string" && phase.trim().length > 0 ? phase : "event";
}

export function createToolContinuationWatchdog(params: ToolContinuationWatchdogParams): {
  onAgentEvent: (evt: ToolContinuationWatchdogAgentEvent) => void;
  clear: (reason: string) => void;
  dispose: () => void;
} {
  const now = params.now ?? Date.now;
  const setTimer = params.setTimer ?? globalThis.setTimeout;
  const clearTimer = params.clearTimer ?? globalThis.clearTimeout;
  const timeoutMs = params.timeoutMs ?? POST_TOOL_CONTINUATION_TIMEOUT_MS;

  let lastAgentEventSeq = 0;
  let lastAgentEventAt = now();
  let lastTerminalToolResult: TerminalToolResultSnapshot | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const clear = (reason: string) => {
    if (timer !== undefined) {
      clearTimer(timer);
      timer = undefined;
    }
    if (!params.isProbeSession && lastTerminalToolResult) {
      params.logDebug?.(
        `embedded run tool continuation watchdog cleared: runId=${params.runId} sessionId=${params.sessionId} ` +
          `reason=${reason} toolCallId=${lastTerminalToolResult.toolCallId} tool=${lastTerminalToolResult.toolName}`,
      );
    }
    lastTerminalToolResult = undefined;
  };

  const arm = (snapshot: TerminalToolResultSnapshot) => {
    clear("rearm");
    lastTerminalToolResult = snapshot;
    timer = setTimer(() => {
      const elapsedMs = now() - snapshot.endedAt;
      const timeoutError = new Error(POST_TOOL_CONTINUATION_TIMEOUT_ERROR_TEXT);
      if (!params.isProbeSession) {
        params.logError?.(
          `embedded run tool continuation timeout: runId=${params.runId} sessionId=${params.sessionId} ` +
            `sessionKey=${params.sessionKey ?? "-"} toolCallId=${snapshot.toolCallId} tool=${snapshot.toolName} ` +
            `elapsedMs=${elapsedMs} lastAgentEventSeq=${lastAgentEventSeq} lastAgentEventAt=${lastAgentEventAt}`,
        );
      }
      void params.emitFailureNote().finally(() => {
        params.onTimeout(timeoutError);
      });
    }, timeoutMs);
    if (!params.isProbeSession) {
      params.logDebug?.(
        `embedded run tool continuation watchdog armed: runId=${params.runId} sessionId=${params.sessionId} ` +
          `toolCallId=${snapshot.toolCallId} tool=${snapshot.toolName} timeoutMs=${timeoutMs}`,
      );
    }
  };

  return {
    onAgentEvent(evt) {
      lastAgentEventSeq += 1;
      lastAgentEventAt = now();
      const terminalToolResult = readTerminalToolResultFromEvent(evt, now);
      if (terminalToolResult) {
        arm(terminalToolResult);
        return;
      }
      if (isContinuationProgressEvent(evt)) {
        clear(`${evt.stream}:${describeContinuationPhase(evt)}`);
      }
    },
    clear,
    dispose() {
      clear("dispose");
    },
  };
}
