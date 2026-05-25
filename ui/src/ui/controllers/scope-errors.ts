import { ConnectErrorDetailCodes } from "../../../../src/gateway/protocol/connect-error-details.js";
import { GatewayRequestError, resolveGatewayErrorDetailCode } from "../gateway.ts";

export function isMissingOperatorScopeError(
  err: unknown,
  scope: "operator.read" | "operator.write",
): boolean {
  if (!(err instanceof GatewayRequestError)) {
    return false;
  }
  const mentionsScope = err.message.includes(`missing scope: ${scope}`);
  if (mentionsScope) {
    return true;
  }
  const mentionsOtherOperatorScope = /missing scope: operator\.(?:read|write|admin)/.test(
    err.message,
  );
  const detailCode = resolveGatewayErrorDetailCode(err);
  // AUTH_UNAUTHORIZED is the current server signal for scope failures in RPC responses.
  // If the server did not include the missing scope in the message, preserve the legacy
  // read-scope fallback used by the existing catalog/effective read calls.
  return (
    scope === "operator.read" &&
    detailCode === ConnectErrorDetailCodes.AUTH_UNAUTHORIZED &&
    !mentionsOtherOperatorScope
  );
}

export function isMissingOperatorReadScopeError(err: unknown): boolean {
  return isMissingOperatorScopeError(err, "operator.read");
}

export function formatMissingOperatorScopeMessage(
  feature: string,
  scope: "operator.read" | "operator.write",
): string {
  return `This connection is missing ${scope}, so ${feature} cannot be loaded yet.`;
}

export function formatMissingOperatorReadScopeMessage(feature: string): string {
  return formatMissingOperatorScopeMessage(feature, "operator.read");
}
