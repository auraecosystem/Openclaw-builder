import type { ExecToolDefaults } from "../../agents/bash-tools.js";
import type { SessionEntry } from "../../config/sessions.js";
import type { InlineDirectives } from "./directive-handling.parse.js";

export type ReplyExecOverrides = Pick<
  ExecToolDefaults,
  "host" | "mode" | "security" | "ask" | "node"
>;

export function resolveReplyExecOverrides(params: {
  directives: InlineDirectives;
  sessionEntry?: SessionEntry;
  agentExecDefaults?: ReplyExecOverrides;
}): ReplyExecOverrides | undefined {
  const directivesHaveLegacyExecPolicy =
    params.directives.execSecurity !== undefined || params.directives.execAsk !== undefined;
  const directivesHaveMode = params.directives.execMode !== undefined;
  const sessionHasLegacyExecPolicy =
    params.sessionEntry?.execSecurity !== undefined || params.sessionEntry?.execAsk !== undefined;
  const host =
    params.directives.execHost ??
    (params.sessionEntry?.execHost as ReplyExecOverrides["host"]) ??
    params.agentExecDefaults?.host;
  const security = directivesHaveMode
    ? undefined
    : (params.directives.execSecurity ??
      (params.sessionEntry?.execSecurity as ReplyExecOverrides["security"]) ??
      params.agentExecDefaults?.security);
  const mode =
    params.directives.execMode ??
    (directivesHaveLegacyExecPolicy
      ? undefined
      : sessionHasLegacyExecPolicy
        ? undefined
        : ((params.sessionEntry?.execMode as ReplyExecOverrides["mode"]) ??
          params.agentExecDefaults?.mode));
  const ask = directivesHaveMode
    ? undefined
    : (params.directives.execAsk ??
      (params.sessionEntry?.execAsk as ReplyExecOverrides["ask"]) ??
      params.agentExecDefaults?.ask);
  const node =
    params.directives.execNode ?? params.sessionEntry?.execNode ?? params.agentExecDefaults?.node;
  if (!host && !mode && !security && !ask && !node) {
    return undefined;
  }
  return { host, mode, security, ask, node };
}
