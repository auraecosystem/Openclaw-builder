import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import AjvPkg from "ajv";
import { vi, afterEach, beforeEach, describe, expect, it } from "vitest";

// Mock openclaw/plugin-sdk imports that config.ts depends on
// These must be in vi.hoisted to ensure they're registered before module resolution
const { mockBuildPluginConfigSchema, mockMapPluginConfigIssues } = vi.hoisted(() => ({
  mockBuildPluginConfigSchema: vi.fn((schemaDef: unknown, { safeParse }: { safeParse: Function }) => ({
    safeParse,
    schema: schemaDef,
  })),
  mockMapPluginConfigIssues: vi.fn((issues: unknown) => issues),
}));

vi.mock("openclaw/plugin-sdk/extension-shared", () => ({
  mapPluginConfigIssues: mockMapPluginConfigIssues,
}));
vi.mock("openclaw/plugin-sdk/plugin-entry", () => ({
  buildPluginConfigSchema: mockBuildPluginConfigSchema,
  definePluginEntry: vi.fn(),
}));
vi.mock("openclaw/plugin-sdk/zod", () => {
  // Re-export actual zod for type validation
  const zod = require("zod");
  return zod;
});
import {
  DEFAULT_PAGE_GROUPS,
  DEFAULT_WIKI_RENDER_MODE,
  DEFAULT_WIKI_SEARCH_BACKEND,
  DEFAULT_WIKI_SEARCH_CORPUS,
  DEFAULT_WIKI_VAULT_MODE,
  findDirForKind,
  buildVaultDirectories,
  getDefaultDirForKind,
  loadExtraPageGroupsFromVault,
  resolveDefaultMemoryWikiVaultPath,
  resolveMemoryWikiConfig,
  WIKI_PAGE_GROUPS_CONFIG_FILENAME,
} from "./config.js";

function compileManifestConfigSchema() {
  const manifest = JSON.parse(
    fs.readFileSync(new URL("../openclaw.plugin.json", import.meta.url), "utf8"),
  ) as { configSchema: Record<string, unknown> };
  const Ajv = AjvPkg as unknown as new (opts?: object) => import("ajv").default;
  const ajv = new Ajv({ allErrors: true, strict: false, useDefaults: true });
  return ajv.compile(manifest.configSchema);
}

describe("resolveMemoryWikiConfig", () => {
  it("returns isolated defaults", () => {
    const config = resolveMemoryWikiConfig(undefined, { homedir: "/Users/tester" });

    expect(config.vaultMode).toBe(DEFAULT_WIKI_VAULT_MODE);
    expect(config.vault.renderMode).toBe(DEFAULT_WIKI_RENDER_MODE);
    expect(config.vault.path).toBe(resolveDefaultMemoryWikiVaultPath("/Users/tester"));
    expect(config.search.backend).toBe(DEFAULT_WIKI_SEARCH_BACKEND);
    expect(config.search.corpus).toBe(DEFAULT_WIKI_SEARCH_CORPUS);
    expect(config.context.includeCompiledDigestPrompt).toBe(false);
  });

  it("expands ~/ paths and preserves explicit modes", () => {
    const config = resolveMemoryWikiConfig(
      {
        vaultMode: "bridge",
        vault: {
          path: "~/vaults/wiki",
          renderMode: "obsidian",
        },
      },
      { homedir: "/Users/tester" },
    );

    expect(config.vaultMode).toBe("bridge");
    expect(config.vault.path).toBe(path.join("/Users/tester", "vaults", "wiki"));
    expect(config.vault.renderMode).toBe("obsidian");
  });

  it("normalizes the bridge artifact toggle", () => {
    const canonical = resolveMemoryWikiConfig({
      bridge: {
        readMemoryArtifacts: false,
      },
    });

    expect(canonical.bridge.readMemoryArtifacts).toBe(false);
  });

  describe("pageGroups", () => {
    let vaultDir = "";

    beforeEach(() => {
      vaultDir = fs.mkdtempSync(path.join(os.tmpdir(), "memory-wiki-pg-test-"));
    });

    afterEach(() => {
      if (vaultDir) {
        fs.rmSync(vaultDir, { recursive: true, force: true });
      }
    });

    it("has correct default pageGroups", () => {
      expect(DEFAULT_PAGE_GROUPS).toEqual([
        { kind: "source", dir: "sources", heading: "Sources" },
        { kind: "entity", dir: "entities", heading: "Entities" },
        { kind: "concept", dir: "concepts", heading: "Concepts" },
        { kind: "synthesis", dir: "syntheses", heading: "Syntheses" },
      ]);
    });

    it("uses DEFAULT_PAGE_GROUPS when no user config or vault file", () => {
      const config = resolveMemoryWikiConfig(
        { vault: { path: vaultDir } },
        { homedir: "/Users/tester" },
      );
      expect(config.pageGroups).toEqual(DEFAULT_PAGE_GROUPS);
    });

    it("appends user-provided pageGroups to defaults", () => {
      const config = resolveMemoryWikiConfig(
        {
          vault: { path: vaultDir },
          pageGroups: [
            { kind: "synthesis", dir: "views", heading: "Views" },
            { kind: "synthesis", dir: "templates", heading: "Templates" },
          ],
        },
        { homedir: "/Users/tester" },
      );

      expect(config.pageGroups).toEqual([
        ...DEFAULT_PAGE_GROUPS,
        { kind: "synthesis", dir: "views", heading: "Views" },
        { kind: "synthesis", dir: "templates", heading: "Templates" },
      ]);
    });

    it("generates fallback heading from dir name when heading omitted", () => {
      const config = resolveMemoryWikiConfig(
        {
          vault: { path: vaultDir },
          pageGroups: [{ kind: "source", dir: "my-special-dir" }],
        },
        { homedir: "/Users/tester" },
      );

      // Last entry should have auto-generated heading
      const extraGroup = config.pageGroups.at(-1);
      expect(extraGroup).toBeDefined();
      expect(extraGroup!.heading).toBe("My-special-dir");
    });

    it("loads extra pageGroups from vault config file", () => {
      const vaultGroups = [
        { kind: "synthesis", dir: "views", heading: "Views" },
      ];
      fs.writeFileSync(
        path.join(vaultDir, WIKI_PAGE_GROUPS_CONFIG_FILENAME),
        JSON.stringify({ pageGroups: vaultGroups }),
        "utf8",
      );

      const config = resolveMemoryWikiConfig(
        { vault: { path: vaultDir } },
        { homedir: "/Users/tester" },
      );

      expect(config.pageGroups).toEqual([
        ...DEFAULT_PAGE_GROUPS,
        ...vaultGroups,
      ]);
    });

    it("merges all three sources: defaults + user config + vault file", () => {
      const vaultGroups = [
        { kind: "synthesis", dir: "views", heading: "Views" },
      ];
      fs.writeFileSync(
        path.join(vaultDir, WIKI_PAGE_GROUPS_CONFIG_FILENAME),
        JSON.stringify({ pageGroups: vaultGroups }),
        "utf8",
      );

      const config = resolveMemoryWikiConfig(
        {
          vault: { path: vaultDir },
          pageGroups: [{ kind: "source", dir: "extras", heading: "Extras" }],
        },
        { homedir: "/Users/tester" },
      );

      expect(config.pageGroups).toEqual([
        ...DEFAULT_PAGE_GROUPS,
        { kind: "source", dir: "extras", heading: "Extras" },
        ...vaultGroups,
      ]);
    });

    it("loadExtraPageGroupsFromVault returns empty array when file missing", () => {
      const result = loadExtraPageGroupsFromVault("/nonexistent/path");
      expect(result).toEqual([]);
    });

    it("loadExtraPageGroupsFromVault returns empty array when JSON is broken", () => {
      fs.writeFileSync(
        path.join(vaultDir, WIKI_PAGE_GROUPS_CONFIG_FILENAME),
        "not valid json",
        "utf8",
      );
      const result = loadExtraPageGroupsFromVault(vaultDir);
      expect(result).toEqual([]);
    });

    it("loadExtraPageGroupsFromVault returns empty array when pageGroups is not an array", () => {
      fs.writeFileSync(
        path.join(vaultDir, WIKI_PAGE_GROUPS_CONFIG_FILENAME),
        JSON.stringify({ pageGroups: "not-an-array" }),
        "utf8",
      );
      const result = loadExtraPageGroupsFromVault(vaultDir);
      expect(result).toEqual([]);
    });

    it("loadExtraPageGroupsFromVault generates fallback heading for vault groups", () => {
      fs.writeFileSync(
        path.join(vaultDir, WIKI_PAGE_GROUPS_CONFIG_FILENAME),
        JSON.stringify({ pageGroups: [{ kind: "source", dir: "raw-data" }] }),
        "utf8",
      );
      const result = loadExtraPageGroupsFromVault(vaultDir);
      expect(result).toEqual([
        { kind: "source", dir: "raw-data", heading: "Raw-data" },
      ]);
    });
  });
});

describe("memory-wiki manifest config schema", () => {
  it("accepts the documented config shape", () => {
    const validate = compileManifestConfigSchema();
    const config = {
      vaultMode: "unsafe-local",
      vault: {
        path: "~/wiki",
        renderMode: "obsidian",
      },
      obsidian: {
        enabled: true,
        useOfficialCli: true,
      },
      bridge: {
        enabled: true,
        readMemoryArtifacts: true,
        followMemoryEvents: true,
      },
      unsafeLocal: {
        allowPrivateMemoryCoreAccess: true,
        paths: ["extensions/memory-core/src"],
      },
      search: {
        backend: "shared",
        corpus: "all",
      },
      context: {
        includeCompiledDigestPrompt: true,
      },
    };

    expect(validate(config)).toBe(true);
  });
});

describe("findDirForKind", () => {
  it("returns dir for a configured kind", () => {
    const groups = [
      { kind: "source" as const, dir: "资料", heading: "资料" },
      { kind: "report" as const, dir: "报告", heading: "报告" },
    ];
    expect(findDirForKind(groups, "report")).toBe("报告");
    expect(findDirForKind(groups, "source")).toBe("资料");
  });

  it("returns null for unconfigured kind", () => {
    const groups = [
      { kind: "source" as const, dir: "资料", heading: "资料" },
    ];
    expect(findDirForKind(groups, "report")).toBeNull();
    expect(findDirForKind(groups, "entity")).toBeNull();
  });

  it("returns first match when multiple entries have same kind", () => {
    const groups = [
      { kind: "report" as const, dir: "report-1", heading: "R1" },
      { kind: "report" as const, dir: "report-2", heading: "R2" },
    ];
    expect(findDirForKind(groups, "report")).toBe("report-1");
  });

  it("returns null for empty pageGroups", () => {
    expect(findDirForKind([], "source")).toBeNull();
  });
});

describe("buildVaultDirectories", () => {
  it("returns pageGroup dirs + system dirs with no duplicates", () => {
    const groups = [
      { kind: "source" as const, dir: "资料", heading: "资料" },
      { kind: "entity" as const, dir: "实体", heading: "实体" },
      { kind: "concept" as const, dir: "概念", heading: "概念" },
      { kind: "synthesis" as const, dir: "综合", heading: "综合" },
      { kind: "report" as const, dir: "报告", heading: "报告" },
    ];
    const dirs = buildVaultDirectories(groups);
    expect(dirs).toContain("资料");
    expect(dirs).toContain("实体");
    expect(dirs).toContain("报告");
    expect(dirs).toContain("_attachments");
    expect(dirs).toContain("_views");
    expect(dirs).toContain(".openclaw-wiki");
    expect(dirs).toContain(".openclaw-wiki/locks");
    expect(dirs).toContain(".openclaw-wiki/cache");
  });

  it("deduplicates when pageGroup dir matches a system dir", () => {
    const groups = [
      { kind: "source" as const, dir: "_attachments", heading: "Attach" },
    ];
    const dirs = buildVaultDirectories(groups);
    const attachmentCount = dirs.filter((d) => d === "_attachments").length;
    expect(attachmentCount).toBe(1);
  });

  it("returns only system dirs when pageGroups is empty", () => {
    const dirs = buildVaultDirectories([]);
    expect(dirs).toEqual([
      "_attachments",
      "_views",
      ".openclaw-wiki",
      ".openclaw-wiki/locks",
      ".openclaw-wiki/cache",
    ]);
  });

  it("includes all pageGroup dirs in the result", () => {
    const groups = [
      { kind: "source" as const, dir: "资料", heading: "资料" },
      { kind: "report" as const, dir: "报告", heading: "报告" },
    ];
    const dirs = buildVaultDirectories(groups);
    expect(dirs).toContain("资料");
    expect(dirs).toContain("报告");
  });
});

describe("getDefaultDirForKind", () => {
  it("returns configured dir when kind is found in pageGroups", () => {
    const groups = [
      { kind: "source" as const, dir: "资料", heading: "资料" },
      { kind: "synthesis" as const, dir: "综合", heading: "综合" },
    ];
    expect(getDefaultDirForKind(groups, "source")).toBe("资料");
    expect(getDefaultDirForKind(groups, "synthesis")).toBe("综合");
  });

  it("falls back to English default when kind is not configured", () => {
    const groups: Array<{ kind: string; dir: string; heading?: string }> = [];
    expect(getDefaultDirForKind([], "source" as any)).toBe("sources");
    expect(getDefaultDirForKind([], "synthesis" as any)).toBe("syntheses");
    expect(getDefaultDirForKind([], "entity" as any)).toBe("entities");
    expect(getDefaultDirForKind([], "concept" as any)).toBe("concepts");
    expect(getDefaultDirForKind([], "report" as any)).toBe("reports");
  });

  it("uses configured dir even for non-standard dir names", () => {
    const groups = [{ kind: "source" as const, dir: "raw-data", heading: "Raw" }];
    expect(getDefaultDirForKind(groups, "source")).toBe("raw-data");
  });
});
