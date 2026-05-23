import type { GatewayBrowserClient } from "../gateway.ts";

export type TraceCapability = {
  available: boolean;
  sourceCheckout: boolean;
  uiEnabled: boolean;
  payloadCaptureEnabled: boolean;
  responseCaptureEnabled: boolean;
  store: "memory";
  reasons: string[];
};

export type LlmTraceSummary = {
  id: string;
  runId: string;
  callId: string;
  sessionKey?: string;
  sessionId?: string;
  provider: string;
  model: string;
  api?: string;
  transport?: string;
  traceId?: string;
  spanId?: string;
  parentSpanId?: string;
  status: "running" | "completed" | "error";
  startedAt: string;
  completedAt?: string;
  durationMs?: number;
  requestPayloadBytes?: number;
  responseStreamBytes?: number;
  timeToFirstByteMs?: number;
  inputItemCount?: number;
  toolCount?: number;
  responseChunkCount: number;
  hasRequestPayload: boolean;
  hasResponseChunks: boolean;
  errorCategory?: string;
  failureKind?: string;
};

export type LlmTraceDetail = LlmTraceSummary & {
  requestPayload?: unknown;
  responseChunks?: unknown[];
};

export type TracesState = {
  client: GatewayBrowserClient | null;
  connected: boolean;
  tracesLoading: boolean;
  tracesError: string | null;
  tracesCapability: TraceCapability | null;
  tracesEntries: LlmTraceSummary[];
  tracesSelectedId: string | null;
  tracesSelected: LlmTraceDetail | null;
  tracesFilterText: string;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function normalizeCapability(value: unknown): TraceCapability {
  const record = isRecord(value) ? value : {};
  return {
    available: record.available === true,
    sourceCheckout: record.sourceCheckout === true,
    uiEnabled: record.uiEnabled === true,
    payloadCaptureEnabled: record.payloadCaptureEnabled === true,
    responseCaptureEnabled: record.responseCaptureEnabled === true,
    store: "memory",
    reasons: Array.isArray(record.reasons)
      ? record.reasons.filter((reason): reason is string => typeof reason === "string")
      : [],
  };
}

function normalizeSummary(value: unknown): LlmTraceSummary | null {
  if (!isRecord(value)) {
    return null;
  }
  const id = typeof value.id === "string" ? value.id : null;
  const runId = typeof value.runId === "string" ? value.runId : null;
  const callId = typeof value.callId === "string" ? value.callId : id;
  const provider = typeof value.provider === "string" ? value.provider : null;
  const model = typeof value.model === "string" ? value.model : null;
  const status =
    value.status === "running" || value.status === "completed" || value.status === "error"
      ? value.status
      : null;
  const startedAt = typeof value.startedAt === "string" ? value.startedAt : null;
  if (!id || !runId || !callId || !provider || !model || !status || !startedAt) {
    return null;
  }
  return {
    id,
    runId,
    callId,
    provider,
    model,
    status,
    startedAt,
    ...(typeof value.sessionKey === "string" ? { sessionKey: value.sessionKey } : {}),
    ...(typeof value.sessionId === "string" ? { sessionId: value.sessionId } : {}),
    ...(typeof value.api === "string" ? { api: value.api } : {}),
    ...(typeof value.transport === "string" ? { transport: value.transport } : {}),
    ...(typeof value.traceId === "string" ? { traceId: value.traceId } : {}),
    ...(typeof value.spanId === "string" ? { spanId: value.spanId } : {}),
    ...(typeof value.parentSpanId === "string" ? { parentSpanId: value.parentSpanId } : {}),
    ...(typeof value.completedAt === "string" ? { completedAt: value.completedAt } : {}),
    ...(typeof value.durationMs === "number" ? { durationMs: value.durationMs } : {}),
    ...(typeof value.requestPayloadBytes === "number"
      ? { requestPayloadBytes: value.requestPayloadBytes }
      : {}),
    ...(typeof value.responseStreamBytes === "number"
      ? { responseStreamBytes: value.responseStreamBytes }
      : {}),
    ...(typeof value.timeToFirstByteMs === "number"
      ? { timeToFirstByteMs: value.timeToFirstByteMs }
      : {}),
    ...(typeof value.inputItemCount === "number" ? { inputItemCount: value.inputItemCount } : {}),
    ...(typeof value.toolCount === "number" ? { toolCount: value.toolCount } : {}),
    responseChunkCount: typeof value.responseChunkCount === "number" ? value.responseChunkCount : 0,
    hasRequestPayload: value.hasRequestPayload === true,
    hasResponseChunks: value.hasResponseChunks === true,
    ...(typeof value.errorCategory === "string" ? { errorCategory: value.errorCategory } : {}),
    ...(typeof value.failureKind === "string" ? { failureKind: value.failureKind } : {}),
  };
}

function normalizeDetail(value: unknown): LlmTraceDetail | null {
  const summary = normalizeSummary(value);
  if (!summary || !isRecord(value)) {
    return summary;
  }
  return {
    ...summary,
    ...(Object.prototype.hasOwnProperty.call(value, "requestPayload")
      ? { requestPayload: value.requestPayload }
      : {}),
    ...(Array.isArray(value.responseChunks) ? { responseChunks: value.responseChunks } : {}),
  };
}

async function requestTraceCapability(state: TracesState): Promise<TraceCapability | null> {
  if (!state.client || !state.connected) {
    return null;
  }
  return normalizeCapability(await state.client.request("traces.capabilities", {}));
}

export async function loadTraceCapability(state: TracesState): Promise<void> {
  try {
    state.tracesCapability = await requestTraceCapability(state);
  } catch {
    state.tracesCapability = {
      available: false,
      payloadCaptureEnabled: false,
      reasons: ["capability_unavailable"],
      responseCaptureEnabled: false,
      sourceCheckout: false,
      store: "memory",
      uiEnabled: false,
    };
  }
}

export async function loadTraceDetail(state: TracesState, id: string): Promise<void> {
  if (!state.client || !state.connected) {
    return;
  }
  const res = await state.client.request("traces.get", { id });
  const record = isRecord(res) ? res.trace : null;
  state.tracesSelected = normalizeDetail(record);
  state.tracesSelectedId = state.tracesSelected?.id ?? id;
}

export async function loadTraces(state: TracesState): Promise<void> {
  if (!state.client || !state.connected || state.tracesLoading) {
    return;
  }
  state.tracesLoading = true;
  state.tracesError = null;
  try {
    const capability = await requestTraceCapability(state);
    if (!capability) {
      return;
    }
    state.tracesCapability = capability;
    if (!capability.available) {
      state.tracesEntries = [];
      state.tracesSelected = null;
      return;
    }
    const listResponse = await state.client.request("traces.list", {});
    const traces =
      isRecord(listResponse) && Array.isArray(listResponse.traces)
        ? listResponse.traces
            .map(normalizeSummary)
            .filter((entry): entry is LlmTraceSummary => Boolean(entry))
        : [];
    state.tracesEntries = traces;
    const selectedId =
      state.tracesSelectedId && traces.some((entry) => entry.id === state.tracesSelectedId)
        ? state.tracesSelectedId
        : (traces[0]?.id ?? null);
    state.tracesSelectedId = selectedId;
    state.tracesSelected = null;
    if (selectedId) {
      await loadTraceDetail(state, selectedId);
    }
  } catch (err) {
    state.tracesError = String(err);
  } finally {
    state.tracesLoading = false;
  }
}
