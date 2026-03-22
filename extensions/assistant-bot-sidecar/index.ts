import type { OpenClawPluginApi } from "openclaw/plugin-sdk";
import { emptyPluginConfigSchema } from "openclaw/plugin-sdk";
import { resolveAssistantBotSidecarConfig } from "./src/config.js";
import { createAssistantBotSidecarService } from "./src/service.js";

const plugin = {
  id: "assistant-bot-sidecar",
  name: "Assistant Bot Sidecar",
  description: "Starts and supervises the trading-tools assistant-bot alongside the gateway",
  configSchema: emptyPluginConfigSchema(),
  register(api: OpenClawPluginApi) {
    const config = resolveAssistantBotSidecarConfig(api.pluginConfig, api.resolvePath);
    api.registerService(createAssistantBotSidecarService(config));
  },
};

export default plugin;
