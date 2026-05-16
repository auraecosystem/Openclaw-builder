import { resolveAgentWorkspaceDir, resolveDefaultAgentId } from "../agents/agent-scope.js";
import { loadConfig } from "../config/config.js";
import { applyPluginAutoEnable } from "../config/plugin-auto-enable.js";
import { createSubsystemLogger } from "../logging.js";
import { loadMoltbotPlugins } from "../plugins/loader.js";
import type { PluginLogger } from "../plugins/types.js";

const log = createSubsystemLogger("plugins");
let pluginRegistryLoaded = false;

export function ensurePluginRegistryLoaded(): void {
  if (pluginRegistryLoaded) return;
  const config = applyPluginAutoEnable({ config: loadConfig(), env: process.env }).config;
  const workspaceDir = resolveAgentWorkspaceDir(config, resolveDefaultAgentId(config));
  const logger: PluginLogger = {
    info: (msg) => log.info(msg),
    warn: (msg) => log.warn(msg),
    error: (msg) => log.error(msg),
    debug: (msg) => log.debug(msg),
  };
  loadMoltbotPlugins({
    config,
    workspaceDir,
    logger,
  });
  pluginRegistryLoaded = true;
}
