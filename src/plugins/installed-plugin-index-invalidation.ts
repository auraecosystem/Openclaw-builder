import { hashJson } from "./installed-plugin-index-hash.js";
import type {
  InstalledPluginIndex,
  InstalledPluginIndexRecord,
  InstalledPluginIndexRefreshReason,
} from "./installed-plugin-index-types.js";

const CONFIG_PATH_SCOPE_COMPAT_CODE = "activation-config-path-hint";

export function hasLegacyInstalledPluginIndexConfigPathScopeGaps(
  index: InstalledPluginIndex,
): boolean {
  return index.plugins.some(
    (plugin) =>
      plugin.compat.includes(CONFIG_PATH_SCOPE_COMPAT_CODE) &&
      plugin.startup.configPaths === undefined,
  );
}

function hasLegacyConfigPathScopeGap(params: {
  previous: InstalledPluginIndexRecord;
  current: InstalledPluginIndexRecord;
}): boolean {
  return (
    params.previous.compat.includes(CONFIG_PATH_SCOPE_COMPAT_CODE) &&
    params.previous.startup.configPaths === undefined &&
    params.current.startup.configPaths !== undefined
  );
}

export function diffInstalledPluginIndexInvalidationReasons(
  previous: InstalledPluginIndex,
  current: InstalledPluginIndex,
): readonly InstalledPluginIndexRefreshReason[] {
  const reasons = new Set<InstalledPluginIndexRefreshReason>();
  if (previous.version !== current.version) {
    reasons.add("missing");
  }
  if (previous.hostContractVersion !== current.hostContractVersion) {
    reasons.add("host-contract-changed");
  }
  if (previous.compatRegistryVersion !== current.compatRegistryVersion) {
    reasons.add("compat-registry-changed");
  }
  if (previous.migrationVersion !== current.migrationVersion) {
    reasons.add("migration");
  }
  if (previous.policyHash !== current.policyHash) {
    reasons.add("policy-changed");
  }
  if (hashJson(previous.installRecords ?? {}) !== hashJson(current.installRecords ?? {})) {
    reasons.add("source-changed");
  }

  const previousByPluginId = new Map(previous.plugins.map((plugin) => [plugin.pluginId, plugin]));
  const currentByPluginId = new Map(current.plugins.map((plugin) => [plugin.pluginId, plugin]));
  for (const [pluginId, previousPlugin] of previousByPluginId) {
    const currentPlugin = currentByPluginId.get(pluginId);
    if (!currentPlugin) {
      reasons.add("source-changed");
      continue;
    }
    if (
      previousPlugin.rootDir !== currentPlugin.rootDir ||
      previousPlugin.manifestPath !== currentPlugin.manifestPath ||
      previousPlugin.installRecordHash !== currentPlugin.installRecordHash
    ) {
      reasons.add("source-changed");
    }
    if (previousPlugin.enabled !== currentPlugin.enabled) {
      reasons.add("policy-changed");
    }
    if (hasLegacyConfigPathScopeGap({ previous: previousPlugin, current: currentPlugin })) {
      reasons.add("migration");
    }
    if (previousPlugin.manifestHash !== currentPlugin.manifestHash) {
      reasons.add("stale-manifest");
    }
    if (
      previousPlugin.packageVersion !== currentPlugin.packageVersion ||
      previousPlugin.packageJson?.path !== currentPlugin.packageJson?.path ||
      previousPlugin.packageJson?.hash !== currentPlugin.packageJson?.hash
    ) {
      reasons.add("stale-package");
    }
  }
  for (const pluginId of currentByPluginId.keys()) {
    if (!previousByPluginId.has(pluginId)) {
      const currentPlugin = currentByPluginId.get(pluginId);
      if (currentPlugin?.enabled === false) {
        continue;
      }
      reasons.add("source-changed");
    }
  }

  return Array.from(reasons).toSorted((left, right) => left.localeCompare(right));
}
