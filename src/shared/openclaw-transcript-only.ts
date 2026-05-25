const TRANSCRIPT_ONLY_OPENCLAW_ASSISTANT_MODELS = new Set(["delivery-mirror", "gateway-injected"]);

export function isTranscriptOnlyOpenClawAssistantMessage(message: unknown): boolean {
  if (!message || typeof message !== "object") {
    return false;
  }
  const record = message as {
    role?: unknown;
    provider?: unknown;
    model?: unknown;
  };
  return (
    record.role === "assistant" &&
    record.provider === "openclaw" &&
    typeof record.model === "string" &&
    TRANSCRIPT_ONLY_OPENCLAW_ASSISTANT_MODELS.has(record.model)
  );
}
