import type { OpenClawConfig } from "../config/types.openclaw.js";
import {
  resolveProviderHookPlugin,
  resolveProviderPluginsForHooks,
} from "./provider-hook-runtime.js";
import type {
  ProviderErrorClassification,
  ProviderFailoverErrorContext,
  ProviderPlugin,
} from "./types.js";

function resolveProviderErrorPlugins(params: {
  provider?: string;
  config?: OpenClawConfig;
  workspaceDir?: string;
  env?: NodeJS.ProcessEnv;
}): ProviderPlugin[] {
  return params.provider
    ? [resolveProviderHookPlugin({ ...params, provider: params.provider })].filter(
        (plugin): plugin is ProviderPlugin => Boolean(plugin),
      )
    : resolveProviderPluginsForHooks(params);
}

function normalizeProviderErrorClassification(
  classification: ProviderErrorClassification | null | undefined,
): ProviderErrorClassification | undefined {
  if (!classification) {
    return undefined;
  }
  if (typeof classification === "string") {
    return classification;
  }
  return classification.reason ? classification : undefined;
}

function getProviderErrorDescriptorReason(classification: ProviderErrorClassification | undefined) {
  if (!classification) {
    return undefined;
  }
  return typeof classification === "string" ? classification : classification.reason;
}

export function matchesProviderContextOverflowWithPlugin(params: {
  provider?: string;
  config?: OpenClawConfig;
  workspaceDir?: string;
  env?: NodeJS.ProcessEnv;
  context: ProviderFailoverErrorContext;
}): boolean {
  for (const plugin of resolveProviderErrorPlugins(params)) {
    if (plugin.matchesContextOverflowError?.(params.context)) {
      return true;
    }
  }
  return false;
}

export function classifyProviderErrorWithPlugin(params: {
  provider?: string;
  config?: OpenClawConfig;
  workspaceDir?: string;
  env?: NodeJS.ProcessEnv;
  context: ProviderFailoverErrorContext;
}): ProviderErrorClassification | undefined {
  for (const plugin of resolveProviderErrorPlugins(params)) {
    const classification = normalizeProviderErrorClassification(
      plugin.classifyProviderError?.(params.context),
    );
    if (classification) {
      return classification;
    }
    const reason = plugin.classifyFailoverReason?.(params.context);
    if (reason) {
      return reason;
    }
  }
  return undefined;
}

export function classifyProviderFailoverReasonWithPlugin(params: {
  provider?: string;
  config?: OpenClawConfig;
  workspaceDir?: string;
  env?: NodeJS.ProcessEnv;
  context: ProviderFailoverErrorContext;
}) {
  return getProviderErrorDescriptorReason(classifyProviderErrorWithPlugin(params));
}
