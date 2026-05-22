import "./fs-safe-defaults.js";
import {
  resolvePathViaExistingAncestorSync as _resolvePathViaExistingAncestorSync,
} from "@openclaw/fs-safe/advanced";

const ancestorPathCache = new Map<string, string>();

export function resolvePathViaExistingAncestorSync(targetPath: string): string {
  const cached = ancestorPathCache.get(targetPath);
  if (cached !== undefined) {
    return cached;
  }
  const result = _resolvePathViaExistingAncestorSync(targetPath);
  ancestorPathCache.set(targetPath, result);
  return result;
}

export {
  ROOT_PATH_ALIAS_POLICIES,
  resolveRootPath,
  resolveRootPathSync,
  type ResolvedRootPath,
  type RootPathAliasPolicy,
} from "@openclaw/fs-safe/advanced";

export function invalidateAncestorPathCache(): void {
  ancestorPathCache.clear();
}
