import { spawnSync } from "node:child_process";
import fs, { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  filterSparseMissingOxlintTargets,
  shouldPrepareExtensionPackageBoundaryArtifacts,
  shouldRunNativeTypeAwareOxlint,
} from "../../scripts/run-oxlint.mjs";
import { createScriptTestHarness } from "./test-helpers.js";

const { createTempDir } = createScriptTestHarness();

describe("run-oxlint", () => {
  it("prepares extension package boundary artifacts for normal lint runs", () => {
    expect(shouldPrepareExtensionPackageBoundaryArtifacts([])).toBe(true);
    expect(shouldPrepareExtensionPackageBoundaryArtifacts(["src/index.ts"])).toBe(true);
    expect(shouldPrepareExtensionPackageBoundaryArtifacts(["--type-aware"])).toBe(true);
  });

  it("treats type-aware oxlint as native type-aware work even with explicit file targets", () => {
    expect(shouldRunNativeTypeAwareOxlint(["--type-aware", "--", "src/index.ts"])).toBe(true);
    expect(shouldRunNativeTypeAwareOxlint(["src/index.ts"])).toBe(false);
    expect(shouldRunNativeTypeAwareOxlint(["--help", "--type-aware"])).toBe(false);
  });

  it("skips artifact preparation for metadata-only oxlint commands", () => {
    expect(shouldPrepareExtensionPackageBoundaryArtifacts(["--help"])).toBe(false);
    expect(shouldPrepareExtensionPackageBoundaryArtifacts(["--version"])).toBe(false);
    expect(shouldPrepareExtensionPackageBoundaryArtifacts(["--print-config"])).toBe(false);
    expect(shouldPrepareExtensionPackageBoundaryArtifacts(["--rules"])).toBe(false);
  });

  it("does not run package-boundary artifact prep twice in pnpm check", () => {
    const packageJson = JSON.parse(readFileSync("package.json", "utf8")) as {
      scripts: Record<string, string>;
    };
    const shardedLintRunner = readFileSync("scripts/run-oxlint-shards.mjs", "utf8");

    expect(packageJson.scripts.check).toBe("node scripts/check.mjs");
    expect(packageJson.scripts.lint).toBe("node scripts/run-oxlint-shards.mjs");
    expect(packageJson.scripts.check).not.toContain(
      "node scripts/prepare-extension-package-boundary-artifacts.mjs",
    );
    expect(shardedLintRunner).toContain("prepare-extension-package-boundary-artifacts.mjs");
    expect(shardedLintRunner).toContain('OPENCLAW_OXLINT_SKIP_PREPARE: "1"');
  });

  it("holds one parent heavy-check lock for sharded lint runs", () => {
    const shardedLintRunner = readFileSync("scripts/run-oxlint-shards.mjs", "utf8");
    const skipLockIndex = shardedLintRunner.indexOf('env.OPENCLAW_OXLINT_SKIP_LOCK === "1"');
    const lockIndex = shardedLintRunner.indexOf("acquireLocalHeavyCheckLockSync({");
    const childSkipIndex = shardedLintRunner.indexOf('OPENCLAW_OXLINT_SKIP_LOCK: "1"');

    expect(shardedLintRunner).toContain("resolveLocalHeavyCheckEnv");
    expect(shardedLintRunner).toContain("shouldAcquireLocalHeavyCheckLockForOxlint");
    expect(skipLockIndex).toBeGreaterThan(-1);
    expect(lockIndex).toBeGreaterThan(-1);
    expect(lockIndex).toBeGreaterThan(skipLockIndex);
    expect(childSkipIndex).toBeGreaterThan(lockIndex);
  });

  it("lets dev update preflight run oxlint shards serially", () => {
    const shardedLintRunner = readFileSync("scripts/run-oxlint-shards.mjs", "utf8");

    expect(shardedLintRunner).toContain("OPENCLAW_OXLINT_SHARDS_SERIAL");
    expect(shardedLintRunner).toContain("runShardsSerial");
  });

  it("filters tracked targets missing from sparse checkouts", () => {
    const result = filterSparseMissingOxlintTargets(
      ["--tsconfig", "config/tsconfig/oxlint.core.json", "src", "ui", "packages", "--threads=1"],
      {
        fileExists: (target: string) => target.endsWith("/src"),
        isSparseCheckoutEnabled: () => true,
        isTrackedPath: ({ target }: { target: string }) => target === "ui" || target === "packages",
      },
    );

    expect(result).toEqual({
      args: ["--tsconfig", "config/tsconfig/oxlint.core.json", "src", "--threads=1"],
      hadExplicitTargets: true,
      remainingExplicitTargets: 1,
      skippedTargets: ["ui", "packages"],
      skippedConfigs: [],
    });
  });

  it("filters tracked tsconfig files missing from sparse checkouts", () => {
    const result = filterSparseMissingOxlintTargets(
      ["--tsconfig", "config/tsconfig/oxlint.core.json", "src"],
      {
        fileExists: (target: string) => target.endsWith("/src"),
        isSparseCheckoutEnabled: () => true,
        isTrackedPath: ({ target }: { target: string }) =>
          target === "config/tsconfig/oxlint.core.json",
      },
    );

    expect(result).toEqual({
      args: ["src"],
      hadExplicitTargets: true,
      remainingExplicitTargets: 1,
      skippedTargets: [],
      skippedConfigs: ["config/tsconfig/oxlint.core.json"],
    });
  });

  it("keeps missing untracked oxlint targets so typos still fail", () => {
    const result = filterSparseMissingOxlintTargets(["src", "typo"], {
      fileExists: (target: string) => target.endsWith("/src"),
      isSparseCheckoutEnabled: () => true,
      isTrackedPath: () => false,
    });

    expect(result).toEqual({
      args: ["src", "typo"],
      hadExplicitTargets: true,
      remainingExplicitTargets: 2,
      skippedTargets: [],
      skippedConfigs: [],
    });
  });

  it("does not create local heavy-check temp directories when sharded oxlint is refused", () => {
    const tmpDir = path.join(createTempDir("openclaw-run-oxlint-refused-"), "heavy-tmp");
    const result = spawnSync(process.execPath, ["scripts/run-oxlint-shards.mjs"], {
      cwd: path.resolve("."),
      encoding: "utf8",
      env: {
        ...process.env,
        CI: "",
        GITHUB_ACTIONS: "",
        OPENCLAW_LOCAL_CHECK_MODE: "",
        OPENCLAW_LOCAL_HEAVY_CHECK_TMPDIR: tmpDir,
      },
    });

    expect(result.status).toBe(1);
    expect(result.stderr).toContain(
      "Refusing to start sharded type-aware oxlint on this local host",
    );
    expect(fs.existsSync(tmpDir)).toBe(false);
  });

  it("delegates sharded metadata-only commands before heavy setup", () => {
    const tmpDir = path.join(createTempDir("openclaw-run-oxlint-metadata-"), "heavy-tmp");
    const result = spawnSync(process.execPath, ["scripts/run-oxlint-shards.mjs", "--help"], {
      cwd: path.resolve("."),
      encoding: "utf8",
      env: {
        ...process.env,
        CI: "",
        GITHUB_ACTIONS: "",
        OPENCLAW_LOCAL_CHECK_MODE: "",
        OPENCLAW_LOCAL_HEAVY_CHECK_TMPDIR: tmpDir,
      },
    });

    expect(result.status).toBe(0);
    expect(result.stderr).not.toContain("prepare-extension-package-boundary-artifacts");
    expect(result.stderr).not.toContain("Refusing to start sharded type-aware oxlint");
    expect(fs.existsSync(tmpDir)).toBe(false);
  });
});
