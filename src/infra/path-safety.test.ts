import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { withTempDir } from "../test-helpers/temp-dir.js";
import {
  invalidateRealpathCaches,
  isPathInsideWithRealpath,
  isWithinDir,
  resolveSafeBaseDir,
  safeRealpathSync,
} from "./path-safety.js";

describe("path-safety", () => {
  it.each([
    { rootDir: "/tmp/demo", expected: `${path.resolve("/tmp/demo")}${path.sep}` },
    { rootDir: `/tmp/demo${path.sep}`, expected: `${path.resolve("/tmp/demo")}${path.sep}` },
    { rootDir: "/tmp/demo/..", expected: `${path.resolve("/tmp")}${path.sep}` },
  ])("resolves safe base dir for %j", ({ rootDir, expected }) => {
    expect(resolveSafeBaseDir(rootDir)).toBe(expected);
  });

  it.each([
    { rootDir: "/tmp/demo", targetPath: "/tmp/demo", expected: true },
    { rootDir: "/tmp/demo", targetPath: "/tmp/demo/sub/file.txt", expected: true },
    { rootDir: "/tmp/demo", targetPath: "/tmp/demo/./nested/../file.txt", expected: true },
    { rootDir: "/tmp/demo", targetPath: "/tmp/demo-two/../demo/file.txt", expected: true },
    { rootDir: "/tmp/demo", targetPath: "/tmp/demo/../escape.txt", expected: false },
    { rootDir: "/tmp/demo", targetPath: "/tmp/demo-sibling/file.txt", expected: false },
    { rootDir: "/tmp/demo", targetPath: "/tmp/demo/../../escape.txt", expected: false },
    { rootDir: "/tmp/demo", targetPath: "sub/file.txt", expected: false },
  ])("checks containment for %j", ({ rootDir, targetPath, expected }) => {
    expect(isWithinDir(rootDir, targetPath)).toBe(expected);
  });
});

describe("realpath cache", () => {
  // Symlink tests require elevated privileges on Windows
  const skipOnWindows = process.platform === "win32";

  it("safeRealpathSync returns consistent results for the same path", async () => {
    if (skipOnWindows) {
      return;
    }
    await withTempDir({ prefix: "openclaw-cache-" }, async (base) => {
      const filePath = path.join(base, "test.txt");
      fs.writeFileSync(filePath, "hello");

      invalidateRealpathCaches();
      const first = safeRealpathSync(filePath);
      const second = safeRealpathSync(filePath);
      expect(first).not.toBeNull();
      expect(first).toBe(second);
    });
  });

  it("invalidateRealpathCaches allows detecting symlink target changes", async () => {
    if (skipOnWindows) {
      return;
    }
    await withTempDir({ prefix: "openclaw-cache-" }, async (base) => {
      const targetA = path.join(base, "target-a");
      const targetB = path.join(base, "target-b");
      const linkPath = path.join(base, "link");

      fs.mkdirSync(targetA);
      fs.mkdirSync(targetB);
      fs.symlinkSync(targetA, linkPath);

      invalidateRealpathCaches();
      const resultA = safeRealpathSync(linkPath);
      expect(resultA).toBe(fs.realpathSync(targetA));

      // Change symlink target
      fs.unlinkSync(linkPath);
      fs.symlinkSync(targetB, linkPath);

      // After invalidation, resolves to new target
      invalidateRealpathCaches();
      const resultB = safeRealpathSync(linkPath);
      expect(resultB).toBe(fs.realpathSync(targetB));
    });
  });

  it("isPathInsideWithRealpath detects escape after symlink change and cache invalidation", async () => {
    if (skipOnWindows) {
      return;
    }
    await withTempDir({ prefix: "openclaw-cache-" }, async (base) => {
      const root = path.join(base, "workspace");
      const safeTarget = path.join(root, "safe");
      const unsafeTarget = path.join(base, "outside");
      const linkPath = path.join(root, "link");

      fs.mkdirSync(safeTarget, { recursive: true });
      fs.mkdirSync(unsafeTarget, { recursive: true });
      fs.symlinkSync(safeTarget, linkPath);

      invalidateRealpathCaches();

      // Symlink points inside root — allowed
      expect(isPathInsideWithRealpath(root, linkPath)).toBe(true);

      // Change symlink to point outside root
      fs.unlinkSync(linkPath);
      fs.symlinkSync(unsafeTarget, linkPath);

      // After invalidation, correctly blocks the escape
      invalidateRealpathCaches();
      expect(isPathInsideWithRealpath(root, linkPath)).toBe(false);
    });
  });
});
