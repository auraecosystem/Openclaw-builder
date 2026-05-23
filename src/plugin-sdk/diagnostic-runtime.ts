// Diagnostic flag/event helpers for plugins that want narrow runtime gating.

export { isDiagnosticFlagEnabled } from "../infra/diagnostic-flags.js";
export type {
  DiagnosticEventMetadata,
  DiagnosticEventPayload,
} from "../infra/diagnostic-events.js";
export {
  emitDiagnosticEvent,
  emitTrustedDiagnosticEvent,
  hasPendingInternalDiagnosticEvent,
  isDiagnosticsEnabled,
  onInternalDiagnosticEvent,
  onDiagnosticEvent,
  resetDiagnosticEventsForTest,
  waitForDiagnosticEventsDrained,
} from "../infra/diagnostic-events.js";
export type { DevLlmTraceModelCall } from "../infra/llm-dev-tracing.js";
export {
  recordDevLlmTraceCompleted,
  recordDevLlmTraceError,
  recordDevLlmTraceRequestPayload,
  recordDevLlmTraceResponseChunk,
  recordDevLlmTraceStarted,
} from "../infra/llm-dev-tracing.js";
export type { DiagnosticTraceContext } from "../infra/diagnostic-trace-context.js";
export {
  createChildDiagnosticTraceContext,
  createDiagnosticTraceContext,
  formatDiagnosticTraceparent,
  isValidDiagnosticSpanId,
  isValidDiagnosticTraceFlags,
  isValidDiagnosticTraceId,
  parseDiagnosticTraceparent,
} from "../infra/diagnostic-trace-context.js";
