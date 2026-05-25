import type { OpenClawConfig } from "../../config/types.openclaw.js";
import { formatErrorMessage } from "../../infra/errors.js";
import type { FailoverReason } from "../pi-embedded-helpers/types.js";
import { buildProviderAuthRecoveryHint } from "../provider-auth-recovery-hint.js";

export type AuthProfileFailureCopyParams = {
  reason: FailoverReason;
  provider: string;
  /**
   * True when the failure was reached because every configured profile is in
   * cooldown / blocked. False when an attempt to use a specific profile threw
   * (e.g. credential lookup failed). The two paths produce different copy
   * because only the cooldown case implies "wait or rotate"; the other case
   * implies "the credential itself is broken".
   */
  allInCooldown: boolean;
  /**
   * Underlying error that triggered the failover, if any. Used to append a
   * short diagnostic suffix and to fall back to the original message when no
   * structured recovery copy applies.
   */
  cause?: unknown;
  config?: OpenClawConfig;
  workspaceDir?: string;
  env?: NodeJS.ProcessEnv;
};

function describeReason(
  reason: FailoverReason,
  provider: string,
  allInCooldown: boolean,
): string | null {
  if (allInCooldown) {
    switch (reason) {
      case "auth":
      case "session_expired":
        return `All auth profiles for ${provider} are unusable (authentication failed or session expired).`;
      case "auth_permanent":
        return `All auth profiles for ${provider} are unusable (authentication permanently denied).`;
      case "billing":
        return `All auth profiles for ${provider} are blocked (billing issue on the provider account).`;
      case "rate_limit":
        return `All auth profiles for ${provider} are cooling down (rate-limited).`;
      case "overloaded":
        return `All auth profiles for ${provider} are cooling down (provider overloaded).`;
      case "timeout":
        return `All auth profiles for ${provider} are cooling down (recent requests timed out).`;
      case "model_not_found":
        return `All auth profiles for ${provider} are cooling down (model not found for any profile).`;
      case "server_error":
        return `All auth profiles for ${provider} are cooling down (provider server errors).`;
      default:
        return `No usable auth profile for ${provider} (all profiles are in cooldown or unavailable).`;
    }
  }
  switch (reason) {
    case "auth":
    case "session_expired":
      return `Authentication failed for ${provider}.`;
    case "auth_permanent":
      return `Authentication permanently denied for ${provider}.`;
    case "billing":
      return `Provider ${provider} reported a billing issue.`;
    default:
      return null;
  }
}

function shouldIncludeRecoveryHint(reason: FailoverReason): boolean {
  switch (reason) {
    case "auth":
    case "auth_permanent":
    case "session_expired":
    case "billing":
      return true;
    case "rate_limit":
    case "overloaded":
    case "timeout":
    case "server_error":
    case "model_not_found":
      return false;
    default:
      return true;
  }
}

function diagnosticSuffix(cause: unknown, primary: string): string | null {
  if (cause === undefined || cause === null) {
    return null;
  }
  const text = formatErrorMessage(cause).trim();
  if (!text || primary.includes(text)) {
    return null;
  }
  return ` (${text})`;
}

/**
 * Single source of truth for user-facing copy when an auth-profile rotation
 * fails. Composes a reason-specific sentence with an actionable next-step
 * derived from the provider's plugin manifest (`buildProviderAuthRecoveryHint`).
 *
 * Falls back to the underlying error's text when the reason maps to nothing
 * actionable, so we never produce worse copy than the raw error.
 */
export function formatAuthProfileFailureMessage(params: AuthProfileFailureCopyParams): string {
  const description = describeReason(params.reason, params.provider, params.allInCooldown);
  if (!description) {
    const causeText = params.cause ? formatErrorMessage(params.cause).trim() : "";
    if (causeText) {
      return causeText;
    }
    return `No usable auth profile for ${params.provider} (all profiles are in cooldown or unavailable).`;
  }
  const hint = shouldIncludeRecoveryHint(params.reason)
    ? buildProviderAuthRecoveryHint({
        provider: params.provider,
        config: params.config,
        workspaceDir: params.workspaceDir,
        env: params.env,
      })
    : null;
  const suffix = diagnosticSuffix(params.cause, description);
  const parts = [description];
  if (hint) {
    parts.push(hint);
  }
  const message = parts.join(" ");
  return suffix ? `${message}${suffix}` : message;
}
