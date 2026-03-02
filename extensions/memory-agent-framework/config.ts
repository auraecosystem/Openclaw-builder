export type AgentFrameworkConfig = {
  cliBin: string;
  apiUrl: string;
  apiKey: string;
  autoRecall: boolean;
  autoCapture: boolean;
  recallLimit: number;
};

function resolveEnvVars(value: string): string {
  return value.replace(/\$\{([^}]+)\}/g, (_, name: string) => {
    const v = process.env[name];
    if (!v) {
      throw new Error(`Environment variable ${name} is not set`);
    }
    return v;
  });
}

export const agentFrameworkConfigSchema = {
  parse(value: unknown): AgentFrameworkConfig {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      throw new Error("agent-framework memory config required");
    }
    const cfg = value as Record<string, unknown>;

    if (typeof cfg.apiUrl !== "string" || !cfg.apiUrl.trim()) {
      throw new Error("apiUrl is required");
    }
    const recallLimit = typeof cfg.recallLimit === "number" ? Math.floor(cfg.recallLimit) : 5;
    if (recallLimit < 1 || recallLimit > 20) {
      throw new Error("recallLimit must be between 1 and 20");
    }

    const rawApiKey = typeof cfg.apiKey === "string" ? cfg.apiKey.trim() : "";

    const cliBin = typeof cfg.cliBin === "string" && cfg.cliBin.trim()
      ? cfg.cliBin.trim()
      : "agent";

    return {
      cliBin,
      apiUrl: cfg.apiUrl.trim().replace(/\/$/, ""),
      apiKey: rawApiKey ? resolveEnvVars(rawApiKey) : "",
      autoRecall: cfg.autoRecall !== false,
      autoCapture: cfg.autoCapture === true,
      recallLimit,
    };
  },
};
