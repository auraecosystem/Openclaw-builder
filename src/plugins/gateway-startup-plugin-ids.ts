import { collectConfiguredAgentHarnessRuntimes } from "../agents/harness-runtimes.js";
import { splitTrailingAuthProfile } from "../agents/model-ref-profile.js";
import {
  listExplicitlyDisabledChannelIdsForConfig,
  listPotentialConfiguredChannelIds,
} from "../channels/config-presence.js";
import { collectConfiguredModelRefs } from "../config/model-refs.js";
import type { OpenClawConfig } from "../config/types.openclaw.js";
import {
  DEFAULT_MEMORY_DREAMING_PLUGIN_ID,
  resolveMemoryDreamingConfig,
  resolveMemoryDreamingPluginConfig,
  resolveMemoryDreamingPluginId,
} from "../memory-host-sdk/dreaming.js";
import { normalizeOptionalLowercaseString } from "../shared/string-coerce.js";
import { hasExplicitChannelConfig } from "./channel-presence-policy.js";
import { collectPluginConfigContractMatches } from "./config-contracts.js";
import { normalizePluginsConfigWithResolver } from "./config-normalization-shared.js";
import { normalizePluginId, resolveEffectivePluginActivationState } from "./config-state.js";
import { isPluginEnabledByDefaultForPlatform } from "./default-enablement.js";
import {
  collectConfiguredSpeechProviderIds,
  normalizeConfiguredSpeechProviderIdForStartup,
} from "./gateway-startup-speech-providers.js";
import { CONFIG_PATH_ACTIVATION_COMPAT_CODE } from "./installed-plugin-index-config-path-scope.js";
import { hashJson } from "./installed-plugin-index-hash.js";
import type { InstalledPluginIndex, InstalledPluginIndexRecord } from "./installed-plugin-index.js";
import type { PluginManifestRecord, PluginManifestRegistry } from "./manifest-registry.js";
import {
  isPluginMetadataSnapshotCompatible,
  resolvePluginMetadataSnapshot,
  type PluginMetadataSnapshot,
} from "./plugin-metadata-snapshot.js";
import type { PluginMetadataSnapshotPluginIdScope } from "./plugin-metadata-snapshot.types.js";
import {
  createPluginRegistryIdNormalizer,
  normalizePluginsConfigWithRegistry,
} from "./plugin-registry-contributions.js";
import type { PluginRegistrySnapshot } from "./plugin-registry-snapshot.js";
import { normalizePluginIdScope } from "./plugin-scope.js";

export type GatewayStartupPluginPlan = {
  channelPluginIds: readonly string[];
  configuredDeferredChannelPluginIds: readonly string[];
  pluginIds: readonly string[];
};

type NormalizedPluginsConfig = ReturnType<typeof normalizePluginsConfigWithRegistry>;
type GenerationProviderContractKey =
  | "imageGenerationProviders"
  | "videoGenerationProviders"
  | "musicGenerationProviders";
type ConfiguredGenerationProviderIds = Record<GenerationProviderContractKey, ReadonlySet<string>>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function sortUniquePluginIds(values: Iterable<string>): string[] {
  return [...new Set([...values].map((value) => value.trim()).filter(Boolean))].toSorted(
    (left, right) => left.localeCompare(right),
  );
}

function createInstalledIndexPluginIdNormalizer(
  index: InstalledPluginIndex,
): (pluginId: string) => string {
  const pluginIdsByLowercase = new Map<string, string>();
  for (const plugin of index.plugins) {
    const normalized = normalizeOptionalLowercaseString(plugin.pluginId);
    if (normalized) {
      pluginIdsByLowercase.set(normalized, plugin.pluginId);
    }
  }
  return (pluginId: string): string => {
    const normalized = normalizePluginId(pluginId);
    const lowercase = normalizeOptionalLowercaseString(normalized);
    return lowercase ? (pluginIdsByLowercase.get(lowercase) ?? normalized) : normalized;
  };
}

function normalizePluginsConfigForInstalledIndex(
  config: OpenClawConfig["plugins"] | undefined,
  index: InstalledPluginIndex,
) {
  return normalizePluginsConfigWithResolver(config, createInstalledIndexPluginIdNormalizer(index));
}

function isConfigActivationValueEnabled(value: unknown): boolean {
  if (value === false) {
    return false;
  }
  if (isRecord(value) && value.enabled === false) {
    return false;
  }
  return true;
}

function listPotentialEnabledChannelIds(config: OpenClawConfig, env: NodeJS.ProcessEnv): string[] {
  const disabled = new Set(listExplicitlyDisabledChannelIdsForConfig(config));
  return listPotentialConfiguredChannelIds(config, env, { includePersistedAuthState: false })
    .map((id) => normalizeOptionalLowercaseString(id) ?? "")
    .filter((id) => id && !disabled.has(id));
}

function isGatewayStartupMemoryPlugin(plugin: InstalledPluginIndexRecord): boolean {
  return plugin.startup.memory;
}

function resolveGatewayStartupDreamingPluginIds(config: OpenClawConfig): Set<string> {
  const dreamingConfig = resolveMemoryDreamingConfig({
    pluginConfig: resolveMemoryDreamingPluginConfig(config),
    cfg: config,
  });
  if (!dreamingConfig.enabled) {
    return new Set();
  }
  return new Set([DEFAULT_MEMORY_DREAMING_PLUGIN_ID, resolveMemoryDreamingPluginId(config)]);
}

function resolveMemorySlotStartupPluginId(params: {
  activationSourceConfig: OpenClawConfig;
  activationSourcePlugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  normalizePluginId: (pluginId: string) => string;
}): string | undefined {
  const { activationSourceConfig, activationSourcePlugins, normalizePluginId } = params;
  const configuredSlot = activationSourceConfig.plugins?.slots?.memory?.trim();
  if (configuredSlot?.toLowerCase() === "none") {
    return undefined;
  }
  if (!configuredSlot) {
    const defaultSlot = activationSourcePlugins.slots.memory;
    if (typeof defaultSlot !== "string") {
      return undefined;
    }
    if (
      activationSourcePlugins.allow.length > 0 &&
      !activationSourcePlugins.allow.includes(defaultSlot)
    ) {
      return undefined;
    }
    return defaultSlot;
  }
  return normalizePluginId(configuredSlot);
}

function resolveContextEngineSlotStartupPluginId(params: {
  activationSourceConfig: OpenClawConfig;
  activationSourcePlugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  normalizePluginId: (pluginId: string) => string;
}): string | undefined {
  const { activationSourceConfig, activationSourcePlugins, normalizePluginId } = params;
  const configuredSlot = activationSourceConfig.plugins?.slots?.contextEngine?.trim();
  if (!configuredSlot) {
    return undefined;
  }
  const normalized = normalizePluginId(configuredSlot);
  // "legacy" is the built-in default engine — no plugin startup needed.
  if (normalized === "legacy") {
    return undefined;
  }
  if (activationSourcePlugins.deny.includes(normalized)) {
    return undefined;
  }
  if (activationSourcePlugins.entries[normalized]?.enabled === false) {
    return undefined;
  }
  return normalized;
}

function shouldConsiderForGatewayStartup(params: {
  plugin: InstalledPluginIndexRecord;
  manifest: PluginManifestRecord | undefined;
  startupDreamingPluginIds: ReadonlySet<string>;
  memorySlotStartupPluginId?: string;
  contextEngineSlotStartupPluginId?: string;
}): boolean {
  if (params.manifest?.activation?.onStartup === true) {
    return true;
  }
  if (params.contextEngineSlotStartupPluginId === params.plugin.pluginId) {
    return true;
  }
  if (!isGatewayStartupMemoryPlugin(params.plugin)) {
    return false;
  }
  if (params.startupDreamingPluginIds.has(params.plugin.pluginId)) {
    return true;
  }
  return params.memorySlotStartupPluginId === params.plugin.pluginId;
}

function hasConfiguredStartupChannel(params: {
  plugin: InstalledPluginIndexRecord;
  manifestLookup: ManifestRegistryLookup;
  configuredChannelIds: ReadonlySet<string>;
}): boolean {
  return listManifestChannelIds(params.manifestLookup, params.plugin.pluginId).some((channelId) =>
    params.configuredChannelIds.has(channelId),
  );
}

type ManifestRegistryLookup = ReadonlyMap<string, PluginManifestRecord>;

function createManifestRegistryLookup(
  manifestRegistry: PluginManifestRegistry,
): ManifestRegistryLookup {
  return new Map(manifestRegistry.plugins.map((plugin) => [plugin.id, plugin]));
}

function listManifestChannelIds(
  manifestLookup: ManifestRegistryLookup,
  pluginId: string,
): readonly string[] {
  return manifestLookup.get(pluginId)?.channels ?? [];
}

function findManifestPlugin(
  manifestLookup: ManifestRegistryLookup,
  pluginId: string,
): PluginManifestRecord | undefined {
  return manifestLookup.get(pluginId);
}

function hasConfiguredActivationPath(params: {
  manifest: PluginManifestRecord | undefined;
  config: OpenClawConfig;
}): boolean {
  return hasConfiguredActivationPathPatterns({
    paths: params.manifest?.activation?.onConfigPaths,
    config: params.config,
  });
}

function hasConfiguredActivationPathPatterns(params: {
  paths: readonly string[] | undefined;
  config: OpenClawConfig;
}): boolean {
  const paths = params.paths;
  if (!paths?.length) {
    return false;
  }
  return paths.some((pathPattern) =>
    collectPluginConfigContractMatches({
      root: params.config,
      pathPattern,
    }).some((match) => isConfigActivationValueEnabled(match.value)),
  );
}

function canUseInstalledIndexConfigPathActivationScope(index: InstalledPluginIndex): boolean {
  return index.plugins.every(
    (plugin) =>
      !plugin.compat.includes(CONFIG_PATH_ACTIVATION_COMPAT_CODE) ||
      plugin.startup.configPaths !== undefined,
  );
}

function addConfiguredActivationPathPluginIds(
  target: Set<string>,
  params: {
    activationSourceConfig: OpenClawConfig;
    index: InstalledPluginIndex;
  },
): void {
  for (const plugin of params.index.plugins) {
    if (plugin.origin !== "bundled") {
      continue;
    }
    if (
      hasConfiguredActivationPathPatterns({
        paths: plugin.startup.configPaths,
        config: params.activationSourceConfig,
      })
    ) {
      target.add(plugin.pluginId);
    }
  }
}

function manifestOwnsConfiguredSpeechProvider(params: {
  manifest: PluginManifestRecord | undefined;
  configuredSpeechProviderIds: ReadonlySet<string>;
}): boolean {
  if (params.configuredSpeechProviderIds.size === 0) {
    return false;
  }
  return (params.manifest?.contracts?.speechProviders ?? []).some((providerId) => {
    const normalized = normalizeConfiguredSpeechProviderIdForStartup(providerId);
    return normalized ? params.configuredSpeechProviderIds.has(normalized) : false;
  });
}

function collectConfiguredWebSearchProviderIds(config: OpenClawConfig): ReadonlySet<string> {
  const search = config.tools?.web?.search;
  if (search?.enabled === false || typeof search?.provider !== "string") {
    return new Set();
  }
  const providerId = normalizeOptionalLowercaseString(search.provider);
  return providerId ? new Set([providerId]) : new Set();
}

function manifestOwnsConfiguredWebSearchProvider(params: {
  manifest: PluginManifestRecord | undefined;
  configuredWebSearchProviderIds: ReadonlySet<string>;
}): boolean {
  if (params.configuredWebSearchProviderIds.size === 0) {
    return false;
  }
  return (params.manifest?.contracts?.webSearchProviders ?? []).some((providerId) => {
    const normalized = normalizeOptionalLowercaseString(providerId);
    return normalized ? params.configuredWebSearchProviderIds.has(normalized) : false;
  });
}

function listModelProviderRefs(value: unknown): string[] {
  if (typeof value === "string") {
    return [value];
  }
  if (!isRecord(value)) {
    return [];
  }
  const refs: string[] = [];
  if (typeof value.primary === "string") {
    refs.push(value.primary);
  }
  if (Array.isArray(value.fallbacks)) {
    for (const fallback of value.fallbacks) {
      if (typeof fallback === "string") {
        refs.push(fallback);
      }
    }
  }
  return refs;
}

function collectModelProviderIds(value: unknown): ReadonlySet<string> {
  return new Set(
    listModelProviderRefs(value)
      .map((ref) => {
        const slashIndex = ref.indexOf("/");
        return slashIndex > 0 ? normalizeOptionalLowercaseString(ref.slice(0, slashIndex)) : "";
      })
      .filter((providerId): providerId is string => Boolean(providerId)),
  );
}

function collectConfiguredGenerationProviderIds(
  config: OpenClawConfig,
): ConfiguredGenerationProviderIds {
  const defaults = config.agents?.defaults;
  return {
    imageGenerationProviders: collectModelProviderIds(defaults?.imageGenerationModel),
    videoGenerationProviders: collectModelProviderIds(defaults?.videoGenerationModel),
    musicGenerationProviders: collectModelProviderIds(defaults?.musicGenerationModel),
  };
}

function addPluginConfigEntryIds(
  target: Set<string>,
  plugins: ReturnType<typeof normalizePluginsConfigForInstalledIndex>,
): void {
  for (const [pluginId, entry] of Object.entries(plugins.entries)) {
    if (entry?.enabled !== false) {
      target.add(pluginId);
    }
  }
}

function addConfiguredSlotPluginIds(
  target: Set<string>,
  params: {
    activationSourceConfig: OpenClawConfig;
    activationSourcePlugins: ReturnType<typeof normalizePluginsConfigForInstalledIndex>;
    normalizePluginId: (pluginId: string) => string;
  },
): void {
  const memorySlot = resolveMemorySlotStartupPluginId(params);
  if (memorySlot) {
    target.add(memorySlot);
  }
  const contextEngineSlot = resolveContextEngineSlotStartupPluginId(params);
  if (contextEngineSlot) {
    target.add(contextEngineSlot);
  }
}

function collectConfiguredStartupChannelIds(params: {
  activationSourceConfig: OpenClawConfig;
  config: OpenClawConfig;
  env: NodeJS.ProcessEnv;
}): string[] {
  return sortUniquePluginIds([
    ...listPotentialEnabledChannelIds(params.config, params.env),
    ...listPotentialEnabledChannelIds(params.activationSourceConfig, params.env),
  ]);
}

function collectValidationHeartbeatTargetChannelIds(config: OpenClawConfig): string[] {
  const channelIds: string[] = [];
  const pushTarget = (target: unknown) => {
    if (typeof target !== "string") {
      return;
    }
    const normalized = normalizeOptionalLowercaseString(target);
    if (!normalized || normalized === "last" || normalized === "none") {
      return;
    }
    channelIds.push(normalized);
  };
  pushTarget(config.agents?.defaults?.heartbeat?.target);
  if (Array.isArray(config.agents?.list)) {
    for (const agent of config.agents.list) {
      pushTarget(agent?.heartbeat?.target);
    }
  }
  return sortUniquePluginIds(channelIds);
}

function collectValidationChannelConfigIds(config: OpenClawConfig): string[] {
  const channels = isRecord(config.channels) ? config.channels : null;
  if (!channels) {
    return [];
  }
  return Object.keys(channels)
    .filter((channelId) => channelId !== "defaults" && channelId !== "modelByChannel")
    .map((channelId) => normalizeOptionalLowercaseString(channelId) ?? "")
    .filter(Boolean)
    .toSorted((left, right) => left.localeCompare(right));
}

function collectConfigValidationChannelIds(params: {
  config: OpenClawConfig;
  env: NodeJS.ProcessEnv;
}): string[] {
  return sortUniquePluginIds([
    ...collectValidationChannelConfigIds(params.config),
    ...collectConfiguredStartupChannelIds({
      config: params.config,
      activationSourceConfig: params.config,
      env: params.env,
    }),
    ...collectValidationHeartbeatTargetChannelIds(params.config),
  ]);
}

function pluginRecordMatchesConfiguredChannelId(
  plugin: InstalledPluginIndexRecord,
  configuredChannelId: string,
): boolean {
  const channelId = normalizeOptionalLowercaseString(configuredChannelId);
  if (!channelId) {
    return false;
  }
  const pluginId = normalizeOptionalLowercaseString(plugin.pluginId);
  if (pluginId && pluginId === channelId) {
    return true;
  }
  const packageChannelId = normalizeOptionalLowercaseString(plugin.packageChannel?.id);
  return packageChannelId === channelId;
}

function pluginRecordMatchesConfiguredChannel(
  plugin: InstalledPluginIndexRecord,
  configuredChannelIds: Iterable<string>,
): boolean {
  for (const channelId of configuredChannelIds) {
    if (pluginRecordMatchesConfiguredChannelId(plugin, channelId)) {
      return true;
    }
  }
  return false;
}

function canUseDirectConfiguredChannelScope(params: {
  configuredChannelIds: readonly string[];
  index: InstalledPluginIndex;
}): boolean {
  if (params.configuredChannelIds.length === 0) {
    return true;
  }
  return params.configuredChannelIds.every((channelId) =>
    params.index.plugins.some((plugin) =>
      pluginRecordMatchesConfiguredChannelId(plugin, channelId),
    ),
  );
}

function addDirectConfiguredChannelPluginIds(
  target: Set<string>,
  params: {
    configuredChannelIds: readonly string[];
    index: InstalledPluginIndex;
  },
): void {
  if (params.configuredChannelIds.length === 0) {
    return;
  }
  const configuredChannelIds = new Set(params.configuredChannelIds);
  for (const plugin of params.index.plugins) {
    if (pluginRecordMatchesConfiguredChannel(plugin, configuredChannelIds)) {
      target.add(plugin.pluginId);
    }
  }
}

function listPluginContributionValues(
  plugin: InstalledPluginIndexRecord,
  key: keyof NonNullable<InstalledPluginIndexRecord["contributions"]>,
): readonly string[] {
  const value = plugin.contributions?.[key];
  return Array.isArray(value) ? value : [];
}

function listPluginContractContributionValues(
  plugin: InstalledPluginIndexRecord,
  key: string,
): readonly string[] {
  const value = plugin.contributions?.contracts?.[key];
  return Array.isArray(value) ? value : [];
}

function pluginContributionContains(
  plugin: InstalledPluginIndexRecord,
  key: keyof NonNullable<InstalledPluginIndexRecord["contributions"]>,
  value: string,
): boolean {
  const normalized = normalizeOptionalLowercaseString(value);
  return Boolean(
    normalized &&
    listPluginContributionValues(plugin, key).some(
      (entry) => normalizeOptionalLowercaseString(entry) === normalized,
    ),
  );
}

function pluginContractContributionContains(
  plugin: InstalledPluginIndexRecord,
  key: string,
  value: string,
): boolean {
  const normalized = normalizeOptionalLowercaseString(value);
  return Boolean(
    normalized &&
    listPluginContractContributionValues(plugin, key).some(
      (entry) => normalizeOptionalLowercaseString(entry) === normalized,
    ),
  );
}

function collectConfiguredProviderIds(config: OpenClawConfig): string[] {
  const configuredWebSearchProviderIds = collectConfiguredWebSearchProviderIds(config);
  const configuredGenerationProviderIds = collectConfiguredGenerationProviderIds(config);
  return sortUniquePluginIds([
    ...collectConfiguredSpeechProviderIds(config),
    ...configuredWebSearchProviderIds,
    ...configuredGenerationProviderIds.imageGenerationProviders,
    ...configuredGenerationProviderIds.videoGenerationProviders,
    ...configuredGenerationProviderIds.musicGenerationProviders,
  ]);
}

function collectValidationConfiguredProviderIds(config: OpenClawConfig): string[] {
  const providerIds: string[] = [];
  const pushProviderId = (value: unknown) => {
    if (typeof value !== "string") {
      return;
    }
    const normalized = normalizeOptionalLowercaseString(value);
    if (normalized) {
      providerIds.push(normalized);
    }
  };
  const profiles = config.auth?.profiles;
  if (profiles && typeof profiles === "object") {
    for (const profile of Object.values(profiles)) {
      if (isRecord(profile)) {
        pushProviderId(profile.provider);
      }
    }
  }
  const providers = config.models?.providers;
  if (providers && typeof providers === "object") {
    for (const providerId of Object.keys(providers)) {
      pushProviderId(providerId);
    }
  }
  for (const ref of collectConfiguredModelRefs(config)) {
    const slashIndex = ref.value.indexOf("/");
    if (slashIndex > 0) {
      pushProviderId(ref.value.slice(0, slashIndex));
    }
  }
  pushProviderId(config.tools?.web?.search?.provider);
  pushProviderId(config.tools?.web?.fetch?.provider);
  return sortUniquePluginIds(providerIds);
}

function collectValidationConfiguredShorthandModelIds(config: OpenClawConfig): string[] {
  return sortUniquePluginIds(
    collectConfiguredModelRefs(config)
      .map((ref) => ref.value)
      .filter((ref) => !ref.includes("/"))
      .map((ref) => splitTrailingAuthProfile(ref).model.trim())
      .filter(Boolean),
  );
}

function canUseDirectConfiguredProviderScope(params: {
  configuredProviderIds: readonly string[];
  index: InstalledPluginIndex;
  scopePluginIds: ReadonlySet<string>;
}): boolean {
  if (params.configuredProviderIds.length === 0) {
    return true;
  }
  const pluginIds = new Set(
    params.index.plugins
      .map((plugin) => normalizeOptionalLowercaseString(plugin.pluginId))
      .filter((pluginId): pluginId is string => Boolean(pluginId)),
  );
  const scopePluginIds = new Set(
    [...params.scopePluginIds]
      .map((pluginId) => normalizeOptionalLowercaseString(pluginId))
      .filter((pluginId): pluginId is string => Boolean(pluginId)),
  );
  return params.configuredProviderIds.every((providerId) => {
    const normalized = normalizeOptionalLowercaseString(providerId);
    return Boolean(normalized && (pluginIds.has(normalized) || scopePluginIds.has(normalized)));
  });
}

function pluginRecordOwnsProviderId(
  plugin: InstalledPluginIndexRecord,
  providerId: string,
): boolean {
  return (
    pluginContributionContains(plugin, "providers", providerId) ||
    pluginContributionContains(plugin, "modelCatalogProviders", providerId) ||
    pluginContributionContains(plugin, "autoEnableProviderIds", providerId) ||
    pluginContractContributionContains(plugin, "externalAuthProviders", providerId) ||
    pluginContractContributionContains(plugin, "embeddingProviders", providerId) ||
    pluginContractContributionContains(plugin, "memoryEmbeddingProviders", providerId) ||
    pluginContractContributionContains(plugin, "speechProviders", providerId) ||
    pluginContractContributionContains(plugin, "realtimeTranscriptionProviders", providerId) ||
    pluginContractContributionContains(plugin, "realtimeVoiceProviders", providerId) ||
    pluginContractContributionContains(plugin, "mediaUnderstandingProviders", providerId) ||
    pluginContractContributionContains(plugin, "meetingNotesSourceProviders", providerId) ||
    pluginContractContributionContains(plugin, "imageGenerationProviders", providerId) ||
    pluginContractContributionContains(plugin, "videoGenerationProviders", providerId) ||
    pluginContractContributionContains(plugin, "musicGenerationProviders", providerId) ||
    pluginContractContributionContains(plugin, "webFetchProviders", providerId) ||
    pluginContractContributionContains(plugin, "webSearchProviders", providerId)
  );
}

function pluginRecordOwnsShorthandModelId(
  plugin: InstalledPluginIndexRecord,
  modelId: string,
): boolean {
  const trimmed = modelId.trim();
  if (!trimmed) {
    return false;
  }
  if (
    listPluginContributionValues(plugin, "modelSupportPrefixes").some((prefix) =>
      trimmed.startsWith(prefix),
    )
  ) {
    return true;
  }
  return listPluginContributionValues(plugin, "modelSupportPatterns").some((pattern) => {
    try {
      return new RegExp(pattern, "u").test(trimmed);
    } catch {
      return false;
    }
  });
}

function scopeOnlyReferencesInstalledPluginIds(params: {
  index: InstalledPluginIndex;
  scope: ReadonlySet<string>;
}): boolean {
  const installedPluginIds = new Set(
    params.index.plugins
      .map((plugin) => normalizeOptionalLowercaseString(plugin.pluginId))
      .filter((pluginId): pluginId is string => Boolean(pluginId)),
  );
  return [...params.scope].every((pluginId) => {
    const normalized = normalizeOptionalLowercaseString(pluginId);
    return Boolean(normalized && installedPluginIds.has(normalized));
  });
}

function addDirectConfiguredProviderPluginIds(
  target: Set<string>,
  params: {
    configuredProviderIds: readonly string[];
    index: InstalledPluginIndex;
  },
): void {
  if (params.configuredProviderIds.length === 0) {
    return;
  }
  const configuredProviderIds = new Set(
    params.configuredProviderIds
      .map((providerId) => normalizeOptionalLowercaseString(providerId))
      .filter((providerId): providerId is string => Boolean(providerId)),
  );
  for (const plugin of params.index.plugins) {
    const pluginId = normalizeOptionalLowercaseString(plugin.pluginId);
    if (pluginId && configuredProviderIds.has(pluginId)) {
      target.add(plugin.pluginId);
    }
  }
}

function addRequiredAgentHarnessPluginIds(
  target: Set<string>,
  params: {
    activationSourceConfig: OpenClawConfig;
    config: OpenClawConfig;
    index: InstalledPluginIndex;
    pluginsConfig: ReturnType<typeof normalizePluginsConfigForInstalledIndex>;
    activationSource: {
      plugins: ReturnType<typeof normalizePluginsConfigForInstalledIndex>;
      rootConfig?: OpenClawConfig;
    };
    env: NodeJS.ProcessEnv;
    platform?: NodeJS.Platform;
  },
): void {
  const requiredAgentHarnessRuntimes = new Set(
    collectConfiguredAgentHarnessRuntimes(params.activationSourceConfig, params.env, {
      includeEnvRuntime: false,
      includeLegacyAgentRuntimes: false,
    }),
  );
  if (requiredAgentHarnessRuntimes.size === 0) {
    return;
  }
  for (const plugin of params.index.plugins) {
    if (
      canStartRequiredAgentHarnessPlugin({
        plugin,
        pluginsConfig: params.pluginsConfig,
        activationSource: params.activationSource,
        config: params.config,
        requiredAgentHarnessRuntimes,
        platform: params.platform,
      })
    ) {
      target.add(plugin.pluginId);
    }
  }
}

export function resolveGatewayStartupMetadataPluginIds(params: {
  config: OpenClawConfig;
  activationSourceConfig?: OpenClawConfig;
  env: NodeJS.ProcessEnv;
  index: InstalledPluginIndex;
  platform?: NodeJS.Platform;
}): string[] | undefined {
  const activationSourceConfig = params.activationSourceConfig ?? params.config;
  const pluginsConfig = normalizePluginsConfigForInstalledIndex(
    params.config.plugins,
    params.index,
  );
  const activationSourcePlugins = normalizePluginsConfigForInstalledIndex(
    activationSourceConfig.plugins,
    params.index,
  );
  if (!pluginsConfig.enabled || !activationSourcePlugins.enabled) {
    return [];
  }
  if (
    params.config.plugins?.bundledDiscovery === "compat" ||
    activationSourceConfig.plugins?.bundledDiscovery === "compat"
  ) {
    return undefined;
  }
  if (pluginsConfig.allow.length === 0 && activationSourcePlugins.allow.length === 0) {
    return undefined;
  }

  const scope = new Set<string>([...pluginsConfig.allow, ...activationSourcePlugins.allow]);
  addPluginConfigEntryIds(scope, pluginsConfig);
  addPluginConfigEntryIds(scope, activationSourcePlugins);

  const normalizePluginId = createInstalledIndexPluginIdNormalizer(params.index);
  addConfiguredSlotPluginIds(scope, {
    activationSourceConfig,
    activationSourcePlugins,
    normalizePluginId,
  });
  for (const pluginId of resolveGatewayStartupDreamingPluginIds(params.config)) {
    scope.add(pluginId);
  }
  if (!canUseInstalledIndexConfigPathActivationScope(params.index)) {
    return undefined;
  }
  addConfiguredActivationPathPluginIds(scope, {
    activationSourceConfig,
    index: params.index,
  });

  const configuredChannelIds = collectConfiguredStartupChannelIds({
    config: params.config,
    activationSourceConfig,
    env: params.env,
  });
  if (!canUseDirectConfiguredChannelScope({ configuredChannelIds, index: params.index })) {
    return undefined;
  }
  addDirectConfiguredChannelPluginIds(scope, {
    configuredChannelIds,
    index: params.index,
  });

  const configuredProviderIds = sortUniquePluginIds([
    ...collectConfiguredProviderIds(params.config),
    ...collectConfiguredProviderIds(activationSourceConfig),
  ]);
  if (
    !canUseDirectConfiguredProviderScope({
      configuredProviderIds,
      index: params.index,
      scopePluginIds: scope,
    })
  ) {
    return undefined;
  }
  addDirectConfiguredProviderPluginIds(scope, {
    configuredProviderIds,
    index: params.index,
  });

  addRequiredAgentHarnessPluginIds(scope, {
    activationSourceConfig,
    config: params.config,
    index: params.index,
    pluginsConfig,
    activationSource: {
      plugins: activationSourcePlugins,
      rootConfig: activationSourceConfig,
    },
    env: params.env,
    platform: params.platform,
  });

  const deniedPluginIds = new Set([...pluginsConfig.deny, ...activationSourcePlugins.deny]);
  for (const pluginId of deniedPluginIds) {
    scope.delete(pluginId);
  }
  for (const [pluginId, entry] of Object.entries(pluginsConfig.entries)) {
    if (entry?.enabled === false) {
      scope.delete(pluginId);
    }
  }
  for (const [pluginId, entry] of Object.entries(activationSourcePlugins.entries)) {
    if (entry?.enabled === false) {
      scope.delete(pluginId);
    }
  }
  if (!scopeOnlyReferencesInstalledPluginIds({ index: params.index, scope })) {
    return undefined;
  }
  return sortUniquePluginIds(scope);
}

export function createGatewayStartupMetadataPluginIdScope(params: {
  config: OpenClawConfig;
  activationSourceConfig?: OpenClawConfig;
  env: NodeJS.ProcessEnv;
  platform?: NodeJS.Platform;
}): PluginMetadataSnapshotPluginIdScope {
  const configuredChannelIds = collectConfiguredStartupChannelIds({
    config: params.config,
    activationSourceConfig: params.activationSourceConfig ?? params.config,
    env: params.env,
  });
  return {
    key: hashJson({
      kind: "gateway-startup",
      config: params.config,
      activationSourceConfig: params.activationSourceConfig ?? null,
      configuredChannelIds,
      platform: params.platform ?? null,
    }),
    resolve: ({ index }) =>
      resolveGatewayStartupMetadataPluginIds({
        config: params.config,
        ...(params.activationSourceConfig !== undefined
          ? { activationSourceConfig: params.activationSourceConfig }
          : {}),
        env: params.env,
        index,
        ...(params.platform !== undefined ? { platform: params.platform } : {}),
      }),
  };
}

function addValidationPluginConfigReferences(
  target: Set<string>,
  params: {
    config: OpenClawConfig;
    pluginsConfig: ReturnType<typeof normalizePluginsConfigForInstalledIndex>;
    normalizePluginId: (pluginId: string) => string;
  },
): void {
  for (const pluginId of params.pluginsConfig.allow) {
    target.add(pluginId);
  }
  for (const pluginId of params.pluginsConfig.deny) {
    target.add(pluginId);
  }
  for (const pluginId of Object.keys(params.pluginsConfig.entries)) {
    target.add(pluginId);
  }
  const rawSlots = isRecord(params.config.plugins?.slots) ? params.config.plugins.slots : {};
  const hasExplicitMemorySlot = Object.prototype.hasOwnProperty.call(rawSlots, "memory");
  const memorySlot = hasExplicitMemorySlot ? params.pluginsConfig.slots.memory : undefined;
  if (typeof memorySlot === "string") {
    target.add(params.normalizePluginId(memorySlot));
  }
  const hasExplicitContextEngineSlot = Object.prototype.hasOwnProperty.call(
    rawSlots,
    "contextEngine",
  );
  const contextEngineSlot = hasExplicitContextEngineSlot
    ? params.pluginsConfig.slots.contextEngine
    : undefined;
  if (typeof contextEngineSlot === "string" && contextEngineSlot !== "legacy") {
    target.add(params.normalizePluginId(contextEngineSlot));
  }
}

function canUseContributionConfiguredChannelScope(params: {
  configuredChannelIds: readonly string[];
  index: InstalledPluginIndex;
}): boolean {
  return params.configuredChannelIds.every((channelId) =>
    params.index.plugins.some(
      (plugin) =>
        pluginRecordMatchesConfiguredChannelId(plugin, channelId) ||
        pluginContributionContains(plugin, "channels", channelId) ||
        pluginContributionContains(plugin, "channelConfigs", channelId),
    ),
  );
}

function addContributionConfiguredChannelPluginIds(
  target: Set<string>,
  params: {
    configuredChannelIds: readonly string[];
    index: InstalledPluginIndex;
  },
): void {
  for (const channelId of params.configuredChannelIds) {
    for (const plugin of params.index.plugins) {
      if (
        pluginRecordMatchesConfiguredChannelId(plugin, channelId) ||
        pluginContributionContains(plugin, "channels", channelId) ||
        pluginContributionContains(plugin, "channelConfigs", channelId)
      ) {
        target.add(plugin.pluginId);
      }
    }
  }
}

function canUseContributionConfiguredProviderScope(params: {
  configuredProviderIds: readonly string[];
  index: InstalledPluginIndex;
}): boolean {
  return params.configuredProviderIds.every((providerId) =>
    params.index.plugins.some(
      (plugin) =>
        normalizeOptionalLowercaseString(plugin.pluginId) ===
          normalizeOptionalLowercaseString(providerId) ||
        pluginRecordOwnsProviderId(plugin, providerId),
    ),
  );
}

function addContributionConfiguredProviderPluginIds(
  target: Set<string>,
  params: {
    configuredProviderIds: readonly string[];
    index: InstalledPluginIndex;
  },
): void {
  for (const providerId of params.configuredProviderIds) {
    const normalizedProviderId = normalizeOptionalLowercaseString(providerId);
    if (!normalizedProviderId) {
      continue;
    }
    for (const plugin of params.index.plugins) {
      if (
        normalizeOptionalLowercaseString(plugin.pluginId) === normalizedProviderId ||
        pluginRecordOwnsProviderId(plugin, providerId)
      ) {
        target.add(plugin.pluginId);
      }
    }
  }
}

function canUseContributionShorthandModelScope(params: {
  modelIds: readonly string[];
  index: InstalledPluginIndex;
}): boolean {
  return params.modelIds.every((modelId) =>
    params.index.plugins.some((plugin) => pluginRecordOwnsShorthandModelId(plugin, modelId)),
  );
}

function addContributionShorthandModelPluginIds(
  target: Set<string>,
  params: {
    modelIds: readonly string[];
    index: InstalledPluginIndex;
  },
): void {
  for (const modelId of params.modelIds) {
    for (const plugin of params.index.plugins) {
      if (pluginRecordOwnsShorthandModelId(plugin, modelId)) {
        target.add(plugin.pluginId);
      }
    }
  }
}

export function resolveConfigValidationMetadataPluginIds(params: {
  config: OpenClawConfig;
  env: NodeJS.ProcessEnv;
  index: InstalledPluginIndex;
  platform?: NodeJS.Platform;
}): string[] | undefined {
  const pluginsConfig = normalizePluginsConfigForInstalledIndex(
    params.config.plugins,
    params.index,
  );
  if (params.config.plugins?.bundledDiscovery === "compat" || pluginsConfig.loadPaths.length > 0) {
    return undefined;
  }

  const scope = new Set<string>();
  const normalizePluginId = createInstalledIndexPluginIdNormalizer(params.index);
  addValidationPluginConfigReferences(scope, {
    config: params.config,
    pluginsConfig,
    normalizePluginId,
  });
  if (!canUseInstalledIndexConfigPathActivationScope(params.index)) {
    return undefined;
  }
  addConfiguredActivationPathPluginIds(scope, {
    activationSourceConfig: params.config,
    index: params.index,
  });

  const configuredChannelIds = collectConfigValidationChannelIds({
    config: params.config,
    env: params.env,
  });
  if (
    !canUseContributionConfiguredChannelScope({
      configuredChannelIds,
      index: params.index,
    })
  ) {
    return undefined;
  }
  addContributionConfiguredChannelPluginIds(scope, {
    configuredChannelIds,
    index: params.index,
  });

  const configuredProviderIds = collectValidationConfiguredProviderIds(params.config);
  if (
    !canUseContributionConfiguredProviderScope({
      configuredProviderIds,
      index: params.index,
    })
  ) {
    return undefined;
  }
  addContributionConfiguredProviderPluginIds(scope, {
    configuredProviderIds,
    index: params.index,
  });

  const configuredShorthandModelIds = collectValidationConfiguredShorthandModelIds(params.config);
  if (
    !canUseContributionShorthandModelScope({
      modelIds: configuredShorthandModelIds,
      index: params.index,
    })
  ) {
    return undefined;
  }
  addContributionShorthandModelPluginIds(scope, {
    modelIds: configuredShorthandModelIds,
    index: params.index,
  });

  addRequiredAgentHarnessPluginIds(scope, {
    activationSourceConfig: params.config,
    config: params.config,
    index: params.index,
    pluginsConfig,
    activationSource: {
      plugins: pluginsConfig,
      rootConfig: params.config,
    },
    env: params.env,
    platform: params.platform,
  });

  if (!scopeOnlyReferencesInstalledPluginIds({ index: params.index, scope })) {
    return undefined;
  }
  return sortUniquePluginIds(scope);
}

export function createConfigValidationMetadataPluginIdScope(params: {
  config: OpenClawConfig;
  env: NodeJS.ProcessEnv;
  platform?: NodeJS.Platform;
}): PluginMetadataSnapshotPluginIdScope {
  const configuredChannelIds = collectConfigValidationChannelIds({
    config: params.config,
    env: params.env,
  });
  const configuredProviderIds = collectValidationConfiguredProviderIds(params.config);
  const configuredShorthandModelIds = collectValidationConfiguredShorthandModelIds(params.config);
  return {
    key: hashJson({
      kind: "config-validation",
      config: params.config,
      configuredChannelIds,
      configuredProviderIds,
      configuredShorthandModelIds,
      platform: params.platform ?? null,
    }),
    resolve: ({ index }) =>
      resolveConfigValidationMetadataPluginIds({
        config: params.config,
        env: params.env,
        index,
        ...(params.platform !== undefined ? { platform: params.platform } : {}),
      }),
  };
}

export function isMetadataSnapshotScopedForGatewayStartup(params: {
  metadataSnapshot: Pick<PluginMetadataSnapshot, "index" | "pluginIds">;
  pluginIdScope: PluginMetadataSnapshotPluginIdScope;
}): boolean {
  const expectedPluginIds = normalizePluginIdScope(
    params.pluginIdScope.resolve({ index: params.metadataSnapshot.index }),
  );
  const snapshotPluginIds = normalizePluginIdScope(params.metadataSnapshot.pluginIds);
  if (expectedPluginIds === undefined || snapshotPluginIds === undefined) {
    return expectedPluginIds === undefined && snapshotPluginIds === undefined;
  }
  if (expectedPluginIds.length === 0) {
    return snapshotPluginIds.length === 0;
  }
  const snapshotPluginIdSet = new Set(snapshotPluginIds);
  return expectedPluginIds.every((pluginId) => snapshotPluginIdSet.has(pluginId));
}

function manifestOwnsConfiguredGenerationProvider(params: {
  manifest: PluginManifestRecord | undefined;
  configuredGenerationProviderIds: ConfiguredGenerationProviderIds;
}): boolean {
  for (const contractKey of [
    "imageGenerationProviders",
    "videoGenerationProviders",
    "musicGenerationProviders",
  ] as const) {
    const configuredProviderIds = params.configuredGenerationProviderIds[contractKey];
    if (configuredProviderIds.size === 0) {
      continue;
    }
    if (
      (params.manifest?.contracts?.[contractKey] ?? []).some((providerId) => {
        const normalized = normalizeOptionalLowercaseString(providerId);
        return normalized ? configuredProviderIds.has(normalized) : false;
      })
    ) {
      return true;
    }
  }
  return false;
}

function canStartConfiguredGenerationProviderPlugin(params: {
  plugin: InstalledPluginIndexRecord;
  manifest: PluginManifestRecord | undefined;
  config: OpenClawConfig;
  pluginsConfig: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  activationSource: {
    plugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
    rootConfig?: OpenClawConfig;
  };
  configuredGenerationProviderIds: ConfiguredGenerationProviderIds;
  platform?: NodeJS.Platform;
}): boolean {
  if (
    !manifestOwnsConfiguredGenerationProvider({
      manifest: params.manifest,
      configuredGenerationProviderIds: params.configuredGenerationProviderIds,
    })
  ) {
    return false;
  }
  if (!params.pluginsConfig.enabled || !params.activationSource.plugins.enabled) {
    return false;
  }
  if (
    params.pluginsConfig.deny.includes(params.plugin.pluginId) ||
    params.activationSource.plugins.deny.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  if (
    params.pluginsConfig.entries[params.plugin.pluginId]?.enabled === false ||
    params.activationSource.plugins.entries[params.plugin.pluginId]?.enabled === false
  ) {
    return false;
  }
  const activationState = resolveEffectivePluginActivationState({
    id: params.plugin.pluginId,
    origin: params.plugin.origin,
    config: params.pluginsConfig,
    rootConfig: params.config,
    enabledByDefault: isPluginEnabledByDefaultForPlatform(params.plugin, params.platform),
    activationSource: params.activationSource,
  });
  return (
    activationState.enabled &&
    (params.plugin.origin === "bundled" || activationState.explicitlyEnabled)
  );
}

function canStartRequiredAgentHarnessPlugin(params: {
  plugin: InstalledPluginIndexRecord;
  pluginsConfig: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  activationSource: {
    plugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
    rootConfig?: OpenClawConfig;
  };
  config: OpenClawConfig;
  requiredAgentHarnessRuntimes: ReadonlySet<string>;
  platform?: NodeJS.Platform;
}): boolean {
  if (
    !params.plugin.startup.agentHarnesses.some((runtime) =>
      params.requiredAgentHarnessRuntimes.has(runtime),
    )
  ) {
    return false;
  }
  if (!params.pluginsConfig.enabled || !params.activationSource.plugins.enabled) {
    return false;
  }
  if (
    params.pluginsConfig.deny.includes(params.plugin.pluginId) ||
    params.activationSource.plugins.deny.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  if (
    params.pluginsConfig.entries[params.plugin.pluginId]?.enabled === false ||
    params.activationSource.plugins.entries[params.plugin.pluginId]?.enabled === false
  ) {
    return false;
  }
  if (
    params.pluginsConfig.allow.length > 0 &&
    !params.pluginsConfig.allow.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  if (
    params.activationSource.plugins.allow.length > 0 &&
    !params.activationSource.plugins.allow.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  const activationState = resolveEffectivePluginActivationState({
    id: params.plugin.pluginId,
    origin: params.plugin.origin,
    config: params.pluginsConfig,
    rootConfig: params.config,
    enabledByDefault: isPluginEnabledByDefaultForPlatform(params.plugin, params.platform),
    activationSource: params.activationSource,
  });
  return activationState.enabled || params.plugin.origin === "bundled";
}

function canStartConfiguredSpeechProviderPlugin(params: {
  plugin: InstalledPluginIndexRecord;
  manifest: PluginManifestRecord | undefined;
  config: OpenClawConfig;
  pluginsConfig: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  activationSource: {
    plugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
    rootConfig?: OpenClawConfig;
  };
  configuredSpeechProviderIds: ReadonlySet<string>;
  platform?: NodeJS.Platform;
}): boolean {
  if (
    !manifestOwnsConfiguredSpeechProvider({
      manifest: params.manifest,
      configuredSpeechProviderIds: params.configuredSpeechProviderIds,
    })
  ) {
    return false;
  }
  if (
    params.pluginsConfig.deny.includes(params.plugin.pluginId) ||
    params.activationSource.plugins.deny.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  if (
    params.pluginsConfig.entries[params.plugin.pluginId]?.enabled === false ||
    params.activationSource.plugins.entries[params.plugin.pluginId]?.enabled === false
  ) {
    return false;
  }
  if (params.plugin.origin === "bundled") {
    return true;
  }
  const activationState = resolveEffectivePluginActivationState({
    id: params.plugin.pluginId,
    origin: params.plugin.origin,
    config: params.pluginsConfig,
    rootConfig: params.config,
    enabledByDefault: isPluginEnabledByDefaultForPlatform(params.plugin, params.platform),
    activationSource: params.activationSource,
  });
  return activationState.enabled && activationState.explicitlyEnabled;
}

function canStartConfiguredWebSearchProviderPlugin(params: {
  plugin: InstalledPluginIndexRecord;
  manifest: PluginManifestRecord | undefined;
  config: OpenClawConfig;
  pluginsConfig: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  activationSource: {
    plugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
    rootConfig?: OpenClawConfig;
  };
  configuredWebSearchProviderIds: ReadonlySet<string>;
  platform?: NodeJS.Platform;
}): boolean {
  if (
    !manifestOwnsConfiguredWebSearchProvider({
      manifest: params.manifest,
      configuredWebSearchProviderIds: params.configuredWebSearchProviderIds,
    })
  ) {
    return false;
  }
  if (!params.pluginsConfig.enabled || !params.activationSource.plugins.enabled) {
    return false;
  }
  if (
    params.pluginsConfig.deny.includes(params.plugin.pluginId) ||
    params.activationSource.plugins.deny.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  if (
    params.pluginsConfig.entries[params.plugin.pluginId]?.enabled === false ||
    params.activationSource.plugins.entries[params.plugin.pluginId]?.enabled === false
  ) {
    return false;
  }
  const activationState = resolveEffectivePluginActivationState({
    id: params.plugin.pluginId,
    origin: params.plugin.origin,
    config: params.pluginsConfig,
    rootConfig: params.config,
    enabledByDefault: isPluginEnabledByDefaultForPlatform(params.plugin, params.platform),
    activationSource: params.activationSource,
  });
  return activationState.enabled;
}

function canStartConfiguredRootPlugin(params: {
  plugin: InstalledPluginIndexRecord;
  manifest: PluginManifestRecord | undefined;
  config: OpenClawConfig;
  pluginsConfig: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  activationSourcePlugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
}): boolean {
  if (params.plugin.origin !== "bundled") {
    return false;
  }
  if (!hasConfiguredActivationPath({ manifest: params.manifest, config: params.config })) {
    return false;
  }
  if (!params.pluginsConfig.enabled || !params.activationSourcePlugins.enabled) {
    return false;
  }
  if (
    params.pluginsConfig.deny.includes(params.plugin.pluginId) ||
    params.activationSourcePlugins.deny.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  if (
    params.pluginsConfig.entries[params.plugin.pluginId]?.enabled === false ||
    params.activationSourcePlugins.entries[params.plugin.pluginId]?.enabled === false
  ) {
    return false;
  }
  return true;
}

function hasExplicitHookPolicyConfig(
  entry: NormalizedPluginsConfig["entries"][string] | undefined,
): boolean {
  return (
    entry?.hooks?.allowConversationAccess === true ||
    entry?.hooks?.allowPromptInjection === true ||
    entry?.hooks?.timeoutMs !== undefined ||
    (entry?.hooks?.timeouts !== undefined && Object.keys(entry.hooks.timeouts).length > 0)
  );
}

function hasHookRuntimeStartupIntent(params: {
  plugin: InstalledPluginIndexRecord;
  manifest: PluginManifestRecord | undefined;
  activationSourcePlugins: NormalizedPluginsConfig;
}): boolean {
  if (params.manifest?.activation?.onCapabilities?.includes("hook")) {
    return true;
  }
  return hasExplicitHookPolicyConfig(
    params.activationSourcePlugins.entries[params.plugin.pluginId],
  );
}

function canStartExplicitHookPlugin(params: {
  plugin: InstalledPluginIndexRecord;
  manifest: PluginManifestRecord | undefined;
  config: OpenClawConfig;
  pluginsConfig: NormalizedPluginsConfig;
  activationSource: {
    plugins: NormalizedPluginsConfig;
    rootConfig?: OpenClawConfig;
  };
  activationSourcePlugins: NormalizedPluginsConfig;
  platform?: NodeJS.Platform;
}): boolean {
  const hasHookPolicyIntent = hasExplicitHookPolicyConfig(
    params.activationSourcePlugins.entries[params.plugin.pluginId],
  );
  if (
    !hasHookRuntimeStartupIntent({
      plugin: params.plugin,
      manifest: params.manifest,
      activationSourcePlugins: params.activationSourcePlugins,
    })
  ) {
    return false;
  }
  if (!params.pluginsConfig.enabled || !params.activationSourcePlugins.enabled) {
    return false;
  }
  if (
    params.pluginsConfig.deny.includes(params.plugin.pluginId) ||
    params.activationSourcePlugins.deny.includes(params.plugin.pluginId)
  ) {
    return false;
  }
  if (
    params.pluginsConfig.entries[params.plugin.pluginId]?.enabled === false ||
    params.activationSourcePlugins.entries[params.plugin.pluginId]?.enabled === false
  ) {
    return false;
  }
  const activationState = resolveEffectivePluginActivationState({
    id: params.plugin.pluginId,
    origin: params.plugin.origin,
    config: params.pluginsConfig,
    rootConfig: params.config,
    enabledByDefault: isPluginEnabledByDefaultForPlatform(params.plugin, params.platform),
    activationSource: params.activationSource,
  });
  return activationState.enabled && (activationState.explicitlyEnabled || hasHookPolicyIntent);
}

function canStartConfiguredChannelPlugin(params: {
  plugin: InstalledPluginIndexRecord;
  config: OpenClawConfig;
  pluginsConfig: ReturnType<typeof normalizePluginsConfigWithRegistry>;
  activationSource: {
    plugins: ReturnType<typeof normalizePluginsConfigWithRegistry>;
    rootConfig?: OpenClawConfig;
  };
  manifestLookup: ManifestRegistryLookup;
  platform?: NodeJS.Platform;
}): boolean {
  if (!params.pluginsConfig.enabled) {
    return false;
  }
  if (params.pluginsConfig.deny.includes(params.plugin.pluginId)) {
    return false;
  }
  if (params.pluginsConfig.entries[params.plugin.pluginId]?.enabled === false) {
    return false;
  }
  const explicitBundledChannelConfig =
    params.plugin.origin === "bundled" &&
    listManifestChannelIds(params.manifestLookup, params.plugin.pluginId).some((channelId) =>
      hasExplicitChannelConfig({
        config: params.activationSource.rootConfig ?? params.config,
        channelId,
      }),
    );
  if (
    params.pluginsConfig.allow.length > 0 &&
    !params.pluginsConfig.allow.includes(params.plugin.pluginId) &&
    !explicitBundledChannelConfig
  ) {
    return false;
  }
  if (params.plugin.origin === "bundled") {
    return true;
  }
  const activationState = resolveEffectivePluginActivationState({
    id: params.plugin.pluginId,
    origin: params.plugin.origin,
    config: params.pluginsConfig,
    rootConfig: params.config,
    enabledByDefault: isPluginEnabledByDefaultForPlatform(params.plugin, params.platform),
    activationSource: params.activationSource,
  });
  return activationState.enabled && activationState.explicitlyEnabled;
}

export function resolveChannelPluginIds(params: {
  config: OpenClawConfig;
  workspaceDir?: string;
  env: NodeJS.ProcessEnv;
}): string[] {
  return [...loadGatewayStartupPluginPlan(params).channelPluginIds];
}

export function resolveChannelPluginIdsFromRegistry(params: {
  manifestRegistry: PluginManifestRegistry;
}): string[] {
  const { manifestRegistry } = params;
  return manifestRegistry.plugins
    .filter((plugin) => plugin.channels.length > 0)
    .map((plugin) => plugin.id);
}

export function resolveConfiguredDeferredChannelPluginIdsFromRegistry(params: {
  config: OpenClawConfig;
  env: NodeJS.ProcessEnv;
  index: PluginRegistrySnapshot;
  manifestRegistry: PluginManifestRegistry;
}): string[] {
  const configuredChannelIds = new Set(listPotentialEnabledChannelIds(params.config, params.env));
  if (configuredChannelIds.size === 0) {
    return [];
  }
  const pluginsConfig = normalizePluginsConfigWithRegistry(params.config.plugins, params.index, {
    manifestRegistry: params.manifestRegistry,
  });
  const activationSource = {
    plugins: pluginsConfig,
    rootConfig: params.config,
  };
  const manifestLookup = createManifestRegistryLookup(params.manifestRegistry);
  return params.index.plugins
    .filter(
      (plugin) =>
        hasConfiguredStartupChannel({
          plugin,
          manifestLookup,
          configuredChannelIds,
        }) &&
        plugin.startup.deferConfiguredChannelFullLoadUntilAfterListen &&
        canStartConfiguredChannelPlugin({
          plugin,
          config: params.config,
          pluginsConfig,
          activationSource,
          manifestLookup,
        }),
    )
    .map((plugin) => plugin.pluginId);
}

export function resolveConfiguredDeferredChannelPluginIds(params: {
  config: OpenClawConfig;
  workspaceDir?: string;
  env: NodeJS.ProcessEnv;
}): string[] {
  return [...loadGatewayStartupPluginPlan(params).configuredDeferredChannelPluginIds];
}

export function resolveGatewayStartupPluginPlanFromRegistry(params: {
  config: OpenClawConfig;
  activationSourceConfig?: OpenClawConfig;
  env: NodeJS.ProcessEnv;
  index: PluginRegistrySnapshot;
  manifestRegistry: PluginManifestRegistry;
  platform?: NodeJS.Platform;
}): GatewayStartupPluginPlan {
  const channelPluginIds = resolveChannelPluginIdsFromRegistry({
    manifestRegistry: params.manifestRegistry,
  });
  const configuredDeferredChannelPluginIds = resolveConfiguredDeferredChannelPluginIdsFromRegistry({
    config: params.config,
    env: params.env,
    index: params.index,
    manifestRegistry: params.manifestRegistry,
  });
  const configuredChannelIds = new Set(listPotentialEnabledChannelIds(params.config, params.env));
  const pluginsConfig = normalizePluginsConfigWithRegistry(params.config.plugins, params.index, {
    manifestRegistry: params.manifestRegistry,
  });
  // Startup must classify allowlist exceptions against the raw config snapshot,
  // not the auto-enabled effective snapshot, or configured-only channels can be
  // misclassified as explicit enablement.
  const activationSourceConfig = params.activationSourceConfig ?? params.config;
  const activationSourcePlugins = normalizePluginsConfigWithRegistry(
    activationSourceConfig.plugins,
    params.index,
    { manifestRegistry: params.manifestRegistry },
  );
  const activationSource = {
    plugins: activationSourcePlugins,
    rootConfig: activationSourceConfig,
  };
  const requiredAgentHarnessRuntimes = new Set(
    collectConfiguredAgentHarnessRuntimes(activationSourceConfig, params.env, {
      includeEnvRuntime: false,
      includeLegacyAgentRuntimes: false,
    }),
  );
  const startupDreamingPluginIds = resolveGatewayStartupDreamingPluginIds(params.config);
  const manifestLookup = createManifestRegistryLookup(params.manifestRegistry);
  const configuredSpeechProviderIds = collectConfiguredSpeechProviderIds(activationSourceConfig);
  const configuredWebSearchProviderIds =
    collectConfiguredWebSearchProviderIds(activationSourceConfig);
  const configuredGenerationProviderIds =
    collectConfiguredGenerationProviderIds(activationSourceConfig);
  const normalizePluginId = createPluginRegistryIdNormalizer(params.index, {
    manifestRegistry: params.manifestRegistry,
  });
  const memorySlotStartupPluginId = resolveMemorySlotStartupPluginId({
    activationSourceConfig,
    activationSourcePlugins,
    normalizePluginId,
  });
  const contextEngineSlotStartupPluginId = resolveContextEngineSlotStartupPluginId({
    activationSourceConfig,
    activationSourcePlugins,
    normalizePluginId,
  });
  const pluginIds = params.index.plugins
    .filter((plugin) => {
      const manifest = findManifestPlugin(manifestLookup, plugin.pluginId);
      if (
        hasConfiguredStartupChannel({
          plugin,
          manifestLookup,
          configuredChannelIds,
        })
      ) {
        return canStartConfiguredChannelPlugin({
          plugin,
          config: params.config,
          pluginsConfig,
          activationSource,
          manifestLookup,
          platform: params.platform,
        });
      }
      if (
        canStartRequiredAgentHarnessPlugin({
          plugin,
          pluginsConfig,
          activationSource,
          config: params.config,
          requiredAgentHarnessRuntimes,
          platform: params.platform,
        })
      ) {
        return true;
      }
      if (
        canStartConfiguredRootPlugin({
          plugin,
          manifest,
          config: activationSourceConfig,
          pluginsConfig,
          activationSourcePlugins,
        })
      ) {
        return true;
      }
      if (
        canStartConfiguredSpeechProviderPlugin({
          plugin,
          manifest,
          config: params.config,
          pluginsConfig,
          activationSource,
          configuredSpeechProviderIds,
          platform: params.platform,
        })
      ) {
        return true;
      }
      if (
        canStartConfiguredWebSearchProviderPlugin({
          plugin,
          manifest,
          config: params.config,
          pluginsConfig,
          activationSource,
          configuredWebSearchProviderIds,
          platform: params.platform,
        })
      ) {
        return true;
      }
      if (
        canStartConfiguredGenerationProviderPlugin({
          plugin,
          manifest,
          config: params.config,
          pluginsConfig,
          activationSource,
          configuredGenerationProviderIds,
          platform: params.platform,
        })
      ) {
        return true;
      }
      if (
        canStartExplicitHookPlugin({
          plugin,
          manifest,
          config: params.config,
          pluginsConfig,
          activationSource,
          activationSourcePlugins,
          platform: params.platform,
        })
      ) {
        return true;
      }
      if (
        !shouldConsiderForGatewayStartup({
          plugin,
          manifest,
          startupDreamingPluginIds,
          memorySlotStartupPluginId,
          contextEngineSlotStartupPluginId,
        })
      ) {
        return false;
      }
      const activationState = resolveEffectivePluginActivationState({
        id: plugin.pluginId,
        origin: plugin.origin,
        config: pluginsConfig,
        rootConfig: params.config,
        enabledByDefault: isPluginEnabledByDefaultForPlatform(plugin, params.platform),
        activationSource,
      });
      if (!activationState.enabled) {
        return false;
      }
      if (plugin.origin !== "bundled") {
        return activationState.explicitlyEnabled;
      }
      return activationState.source === "explicit" || activationState.source === "default";
    })
    .map((plugin) => plugin.pluginId);
  return {
    channelPluginIds,
    configuredDeferredChannelPluginIds,
    pluginIds,
  };
}

export function resolveGatewayStartupPluginIdsFromRegistry(params: {
  config: OpenClawConfig;
  activationSourceConfig?: OpenClawConfig;
  env: NodeJS.ProcessEnv;
  index: PluginRegistrySnapshot;
  manifestRegistry: PluginManifestRegistry;
  platform?: NodeJS.Platform;
}): string[] {
  return [...resolveGatewayStartupPluginPlanFromRegistry(params).pluginIds];
}

export function loadGatewayStartupPluginPlan(params: {
  config: OpenClawConfig;
  activationSourceConfig?: OpenClawConfig;
  workspaceDir?: string;
  env: NodeJS.ProcessEnv;
  index?: PluginRegistrySnapshot;
  metadataSnapshot?: PluginMetadataSnapshot;
  platform?: NodeJS.Platform;
}): GatewayStartupPluginPlan {
  const snapshotConfig = params.activationSourceConfig ?? params.config;
  const pluginIdScope = createGatewayStartupMetadataPluginIdScope({
    config: params.config,
    ...(params.activationSourceConfig !== undefined
      ? { activationSourceConfig: params.activationSourceConfig }
      : {}),
    env: params.env,
    ...(params.platform !== undefined ? { platform: params.platform } : {}),
  });
  const metadataSnapshot =
    params.metadataSnapshot &&
    isPluginMetadataSnapshotCompatible({
      snapshot: params.metadataSnapshot,
      config: snapshotConfig,
      env: params.env,
      allowScopedSnapshot: true,
      workspaceDir: params.workspaceDir,
      index: params.index,
    }) &&
    isMetadataSnapshotScopedForGatewayStartup({
      metadataSnapshot: params.metadataSnapshot,
      pluginIdScope,
    })
      ? params.metadataSnapshot
      : resolvePluginMetadataSnapshot({
          config: snapshotConfig,
          workspaceDir: params.workspaceDir,
          env: params.env,
          allowWorkspaceScopedCurrent: params.workspaceDir === undefined,
          ...(params.index ? { index: params.index } : {}),
          pluginIdScope,
        });
  return resolveGatewayStartupPluginPlanFromRegistry({
    config: params.config,
    ...(params.activationSourceConfig !== undefined
      ? { activationSourceConfig: params.activationSourceConfig }
      : {}),
    env: params.env,
    index: metadataSnapshot.index,
    manifestRegistry: metadataSnapshot.manifestRegistry,
    platform: params.platform,
  });
}

export function resolveGatewayStartupPluginIds(params: {
  config: OpenClawConfig;
  activationSourceConfig?: OpenClawConfig;
  workspaceDir?: string;
  env: NodeJS.ProcessEnv;
  platform?: NodeJS.Platform;
}): string[] {
  return [...loadGatewayStartupPluginPlan(params).pluginIds];
}
