import {
  getDevLlmTrace,
  listDevLlmTraces,
  resolveDevLlmTraceConfig,
} from "../../infra/llm-dev-tracing.js";
import { ErrorCodes, errorShape } from "../protocol/index.js";
import type { GatewayClient, GatewayRequestHandlers } from "./types.js";

function traceCapabilityForClient(client: GatewayClient | null) {
  const base = resolveDevLlmTraceConfig();
  const localTraceClient = client?.isLocalClient === true && client.allowsDevLlmTracing === true;
  const reasons = [...base.reasons];
  if (!localTraceClient) {
    reasons.push("not_local_trace_ui_client");
  }
  return {
    ...base,
    available: base.available && localTraceClient,
    payloadCaptureEnabled: base.payloadCaptureEnabled && localTraceClient,
    responseCaptureEnabled: base.responseCaptureEnabled && localTraceClient,
    reasons,
  };
}

function unavailableError(client: GatewayClient | null) {
  const capability = traceCapabilityForClient(client);
  return errorShape(
    ErrorCodes.UNAVAILABLE,
    `LLM tracing is unavailable: ${capability.reasons.join(", ") || "disabled"}`,
  );
}

function readString(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

export const tracesHandlers: GatewayRequestHandlers = {
  "traces.capabilities": async ({ client, respond }) => {
    respond(true, traceCapabilityForClient(client), undefined);
  },

  "traces.list": async ({ client, respond }) => {
    if (!traceCapabilityForClient(client).available) {
      respond(false, undefined, unavailableError(client));
      return;
    }
    respond(true, { traces: listDevLlmTraces() }, undefined);
  },

  "traces.get": async ({ params, client, respond }) => {
    if (!traceCapabilityForClient(client).available) {
      respond(false, undefined, unavailableError(client));
      return;
    }
    const id = readString((params as { id?: unknown } | undefined)?.id);
    if (!id) {
      respond(false, undefined, errorShape(ErrorCodes.INVALID_REQUEST, "id is required"));
      return;
    }
    const trace = getDevLlmTrace(id);
    if (!trace) {
      respond(false, undefined, errorShape(ErrorCodes.INVALID_REQUEST, "trace not found"));
      return;
    }
    respond(true, { trace }, undefined);
  },
};
