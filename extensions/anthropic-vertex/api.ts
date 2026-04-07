import type { ModelDefinitionConfig } from "openclaw/plugin-sdk/provider-model-shared";
export {
  ANTHROPIC_VERTEX_DEFAULT_MODEL_ID,
  buildAnthropicVertexProvider,
} from "./provider-catalog.js";
export {
  hasAnthropicVertexAvailableAuth,
  hasAnthropicVertexCredentials,
  resolveAnthropicVertexClientRegion,
  resolveAnthropicVertexConfigApiKey,
  resolveAnthropicVertexProjectId,
  resolveAnthropicVertexRegion,
  resolveAnthropicVertexRegionFromBaseUrl,
} from "./region.js";
import { buildAnthropicVertexProvider } from "./provider-catalog.js";
import { hasAnthropicVertexAvailableAuth } from "./region.js";

type AnthropicVertexProviderConfig = {
  baseUrl: string;
  api?:
    | "anthropic-messages"
    | "azure-openai-responses"
    | "bedrock-converse-stream"
    | "github-copilot"
    | "google-generative-ai"
    | "ollama"
    | "openai-codex-responses"
    | "openai-completions"
    | "openai-responses";
  apiKey?:
    | string
    | {
        source: "env" | "file" | "exec";
        provider: string;
        id: string;
      };
  models: ModelDefinitionConfig[];
};

export function mergeImplicitAnthropicVertexProvider(params: {
  existing: AnthropicVertexProviderConfig | undefined;
  implicit: AnthropicVertexProviderConfig;
}): AnthropicVertexProviderConfig {
  const { existing, implicit } = params;
  if (!existing) {
    return implicit;
  }
  return {
    ...implicit,
    ...existing,
    models:
      Array.isArray(existing.models) && existing.models.length > 0
        ? existing.models
        : implicit.models,
  };
}

export function resolveImplicitAnthropicVertexProvider(params?: {
  env?: NodeJS.ProcessEnv;
}): AnthropicVertexProviderConfig | null {
  const env = params?.env ?? process.env;
  if (!hasAnthropicVertexAvailableAuth(env)) {
    return null;
  }

  return buildAnthropicVertexProvider({ env });
}
