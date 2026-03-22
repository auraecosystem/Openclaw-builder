import type { OpenClawPluginApi } from "openclaw/plugin-sdk";
import { resolveAssistantBotSidecarConfig } from "./src/config.js";
import { createAssistantBotSidecarService } from "./src/service.js";

const managedProcessSchema = {
  type: "object",
  additionalProperties: false,
  properties: {
    enabled: { type: "boolean" },
    cwd: { type: "string" },
    command: { type: "string" },
    args: {
      type: "array",
      items: { type: "string" },
    },
    envFiles: {
      type: "array",
      items: { type: "string" },
    },
    env: {
      type: "object",
      additionalProperties: { type: "string" },
    },
    restartDelayMs: { type: "number" },
    shutdownGraceMs: { type: "number" },
    watch: {
      type: "object",
      additionalProperties: false,
      properties: {
        enabled: { type: "boolean" },
        debounceMs: { type: "number" },
        paths: {
          type: "array",
          items: { type: "string" },
        },
      },
    },
  },
} as const;

const plugin = {
  id: "assistant-bot-sidecar",
  name: "Assistant Bot Sidecar",
  description:
    "Starts and supervises the trading-tools daemon and assistant-bot alongside the gateway",
  configSchema: {
    type: "object",
    additionalProperties: false,
    properties: {
      enabled: { type: "boolean" },
      startupGraceMs: { type: "number" },
      daemon: managedProcessSchema,
      assistantBot: managedProcessSchema,
      sharedWatchPaths: {
        type: "array",
        items: { type: "string" },
      },
    },
  },
  register(api: OpenClawPluginApi) {
    const config = resolveAssistantBotSidecarConfig(api.pluginConfig, api.resolvePath);
    api.registerService(createAssistantBotSidecarService(config));
  },
};

export default plugin;
