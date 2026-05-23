// Shared tool-risk constants.
// Keep these centralized so gateway HTTP restrictions and security audits don't drift.

/**
 * Tools denied via Gateway HTTP `POST /tools/invoke` by default.
 * These are high-risk because they enable session orchestration, control-plane actions,
 * or interactive flows that don't make sense over a non-interactive HTTP surface.
 */
export const DEFAULT_GATEWAY_HTTP_TOOL_DENY = [
  // Direct command execution — immediate RCE surface
  "exec",
  // Arbitrary child process creation — immediate RCE surface
  "spawn",
  // Shell command execution — immediate RCE surface
  "shell",
  // Arbitrary file mutation on the host
  "fs_write",
  // Arbitrary file deletion on the host
  "fs_delete",
  // Arbitrary file move/rename on the host
  "fs_move",
  // Patch application can rewrite arbitrary files
  "apply_patch",
  // Session orchestration — spawning agents remotely is RCE
  "sessions_spawn",
  // Cross-session injection — message injection across sessions
  "sessions_send",
  // Persistent automation control plane — can create/update/remove scheduled runs
  "cron",
  // Gateway control plane — prevents gateway reconfiguration via HTTP
  "gateway",
  // Node command relay can reach system.run on paired hosts
  "nodes",
  // Host filesystem read access (via the coding `read` tool wired by
  // openclaw/openclaw#85664). Distinct from the entries above: the threat
  // shape is information disclosure of file contents reachable by the
  // gateway process (config files, secrets, SSH keys, environment files,
  // etc.), not RCE / session orchestration. By default `read` is NOT
  // confined to the workspace — `tools.fs.workspaceOnly` is a separate
  // umbrella that the operator may enable. Default-deny so the operator
  // must explicitly accept the host file-read surface by setting
  // `gateway.tools.allow: ["read"]`. Existing `gateway.tools.{allow,deny}`
  // and agent-level tool policies continue to apply on top of this.
  "read",
] as const;
