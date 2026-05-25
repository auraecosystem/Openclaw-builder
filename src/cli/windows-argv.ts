import fs from "node:fs";
import path from "node:path";
import { normalizeLowercaseStringOrEmpty } from "../shared/string-coerce.js";

export function normalizeWindowsArgv(
  argv: string[],
  options: {
    platform?: NodeJS.Platform;
    execPath?: string;
    pathExists?: (path: string) => boolean;
  } = {},
): string[] {
  const platform = options.platform ?? process.platform;
  if (platform !== "win32") {
    return argv;
  }
  if (argv.length < 2) {
    return argv;
  }
  const pathModule = path.win32;
  const pathExists = options.pathExists ?? fs.existsSync;

  const stripControlChars = (value: string): string => {
    let out = "";
    for (let i = 0; i < value.length; i += 1) {
      const code = value.charCodeAt(i);
      if (code >= 32 && code !== 127) {
        out += value[i];
      }
    }
    return out;
  };

  const normalizeArg = (value: string): string =>
    stripControlChars(value)
      .replace(/^['"]+|['"]+$/g, "")
      .trim();
  const normalizeCandidate = (value: string): string =>
    normalizeArg(value).replace(/^\\\\\\?\\/, "");

  const execPath = normalizeCandidate(options.execPath ?? process.execPath);
  const execPathLower = normalizeLowercaseStringOrEmpty(execPath);
  const execBase = normalizeLowercaseStringOrEmpty(pathModule.basename(execPath));
  const isExecPath = (value: string | undefined): boolean => {
    if (!value) {
      return false;
    }
    const normalized = normalizeCandidate(value);
    if (!normalized) {
      return false;
    }
    const lower = normalizeLowercaseStringOrEmpty(normalized);
    return (
      lower === execPathLower ||
      pathModule.basename(lower) === execBase ||
      lower.endsWith("\\node.exe") ||
      lower.endsWith("/node.exe") ||
      lower.includes("node.exe") ||
      (pathModule.basename(lower) === "node.exe" && pathExists(normalized))
    );
  };

  const next = [...argv];
  for (let i = 1; i <= 3 && i < next.length; ) {
    if (isExecPath(next[i])) {
      next.splice(i, 1);
      continue;
    }
    i += 1;
  }
  const filtered = next.filter((arg, index) => index === 0 || !isExecPath(arg));
  if (filtered.length < 3) {
    return filtered;
  }
  const cleaned = [...filtered];
  for (let i = 2; i < cleaned.length; ) {
    const arg = cleaned[i];
    if (!arg || arg.startsWith("-")) {
      i += 1;
      continue;
    }
    if (isExecPath(arg)) {
      cleaned.splice(i, 1);
      continue;
    }
    break;
  }
  return cleaned;
}
