import "./fs-safe-defaults.js";
import {
  safeRealpathSync as _safeRealpathSync,
  isPathInside as _isPathInside,
} from "@openclaw/fs-safe/path";

const globalRealpathCache = new Map<string, string>();

export function safeRealpathSync(
  targetPath: string,
  cache?: Map<string, string>,
): string | null {
  const globalCached = globalRealpathCache.get(targetPath);
  if (globalCached) {
    cache?.set(targetPath, globalCached);
    return globalCached;
  }
  const localCached = cache?.get(targetPath);
  if (localCached) {
    globalRealpathCache.set(targetPath, localCached);
    globalRealpathCache.set(localCached, localCached);
    return localCached;
  }
  const result = _safeRealpathSync(targetPath, cache);
  if (result) {
    globalRealpathCache.set(targetPath, result);
    globalRealpathCache.set(result, result);
    cache?.set(targetPath, result);
    cache?.set(result, result);
  }
  return result;
}

export function isPathInsideWithRealpath(
  basePath: string,
  candidatePath: string,
  opts?: { requireRealpath?: boolean; cache?: Map<string, string> },
): boolean {
  if (!_isPathInside(basePath, candidatePath)) {
    return false;
  }
  const baseReal = safeRealpathSync(basePath, opts?.cache);
  const candidateReal = safeRealpathSync(candidatePath, opts?.cache);
  if (!baseReal || !candidateReal) {
    return opts?.requireRealpath === false;
  }
  return _isPathInside(baseReal, candidateReal);
}

export {
  isNotFoundPathError,
  hasNodeErrorCode,
  isNodeError,
  isPathInside,
  isSymlinkOpenError,
  isWithinDir,
  normalizeWindowsPathForComparison,
  resolveSafeBaseDir,
  resolveSafeRelativePath,
  safeStatSync,
  splitSafeRelativePath,
} from "@openclaw/fs-safe/path";
export { formatPosixMode } from "@openclaw/fs-safe/advanced";

export function invalidateRealpathCaches(): void {
  globalRealpathCache.clear();
}
