export function resolveReplayableResponsesMessageId(params: {
  replayResponsesItemIds: boolean;
  textSignatureId?: string;
  fallbackId: string;
  previousReplayItemWasReasoning: boolean;
}): string | undefined {
  if (!params.replayResponsesItemIds) {
    return undefined;
  }
  if (!params.textSignatureId) {
    return params.fallbackId;
  }
  return params.previousReplayItemWasReasoning ? params.textSignatureId : undefined;
}
