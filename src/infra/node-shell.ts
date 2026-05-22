import { normalizeLowercaseStringOrEmpty } from "../shared/string-coerce.js";

export function buildNodeShellCommand(command: string, platform?: string | null) {
  const normalized = normalizeLowercaseStringOrEmpty((platform ?? "").trim());
  if (normalized.startsWith("win")) {
    return ["cmd.exe", "/d", "/s", "/c", command];
  }
  return ["/bin/sh", "-lc", command];
}

export function buildNodeShellFallbackCommands(argv: string[]): string[][] {
  const executable = argv[0];
  if (executable !== "/bin/sh") {
    return [];
  }
  return [["/usr/bin/sh", ...argv.slice(1)]];
}
