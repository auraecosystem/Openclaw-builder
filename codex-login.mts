import { loginOpenAICodex } from "@mariozechner/pi-ai";
import { exec } from "child_process";
import { createInterface } from "readline";
import { readFileSync, writeFileSync, existsSync } from "fs";
import { join } from "path";
import { homedir } from "os";

const rl = createInterface({ input: process.stdin, output: process.stdout });
const ask = (q: string): Promise<string> =>
  new Promise((res) => rl.question(q, res));

type CodexPrompt = {
  message: string;
};

function resolveAccountId(creds: object): string | undefined {
  if (!("accountId" in creds) || typeof creds.accountId !== "string") {
    return undefined;
  }
  const trimmed = creds.accountId.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function formatUnknownError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

console.log("Starting Codex OAuth flow...\n");

try {
  const creds = await loginOpenAICodex({
    onAuth: async (info: { url: string; instructions?: string }) => {
      console.log("Opening browser for OpenAI sign-in...");
      console.log("URL:", info.url);
      exec(`open "${info.url}"`);
    },
    onPrompt: async (opts: CodexPrompt) => {
      const answer = await ask(`${opts.message}\n> `);
      return answer;
    },
    onProgress: (msg: string) => {
      console.log("  ", msg);
    },
  });

  if (creds) {
    const authPath = join(homedir(), ".openclaw/agents/main/agent/auth-profiles.json");
    const store = JSON.parse(readFileSync(authPath, "utf-8"));

    store.profiles["openai-codex:default"] = {
      type: "oauth",
      provider: "openai-codex",
      access: creds.access,
      refresh: creds.refresh,
      expires: creds.expires,
      accountId: resolveAccountId(creds),
    };

    writeFileSync(authPath, JSON.stringify(store, null, 2));
    console.log("\nCredentials saved to", authPath);
    console.log("Token expires:", new Date(creds.expires).toISOString());

    const gamePmPath = join(homedir(), ".openclaw/agents/game-pm/agent/auth-profiles.json");
    if (existsSync(gamePmPath)) {
      const gamePmStore = JSON.parse(readFileSync(gamePmPath, "utf-8"));
      gamePmStore.profiles["openai-codex:default"] = store.profiles["openai-codex:default"];
      writeFileSync(gamePmPath, JSON.stringify(gamePmStore, null, 2));
      console.log("Also synced to game-pm agent.");
    }

    console.log("\nDone! Codex login refreshed.");
  } else {
    console.log("No credentials returned.");
  }
} catch (e: unknown) {
  console.error("Failed:", formatUnknownError(e));
} finally {
  rl.close();
}
