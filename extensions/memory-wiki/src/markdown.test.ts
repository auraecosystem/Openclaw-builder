import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import {
  createWikiPageFilename,
  inferWikiPageKind,
  renderWikiMarkdown,
  slugifyWikiSegment,
  toWikiPageSummary,
} from "./markdown.js";
import type { WikiPageGroup } from "./config.js";

const DEFAULT_PAGE_GROUPS: WikiPageGroup[] = [
  { kind: "source", dir: "sources", heading: "Sources" },
  { kind: "entity", dir: "entities", heading: "Entities" },
  { kind: "concept", dir: "concepts", heading: "Concepts" },
  { kind: "synthesis", dir: "syntheses", heading: "Syntheses" },
];

describe("slugifyWikiSegment", () => {
  it("preserves Unicode letters and numbers in wiki slugs", () => {
    expect(slugifyWikiSegment("大语言模型概述")).toBe("大语言模型概述");
    expect(slugifyWikiSegment("LLM 架构分析")).toBe("llm-架构分析");
    expect(slugifyWikiSegment("Circuit Breaker 自動恢復")).toBe("circuit-breaker-自動恢復");
  });

  it("keeps ASCII behavior unchanged", () => {
    expect(slugifyWikiSegment("hello world")).toBe("hello-world");
    expect(slugifyWikiSegment("")).toBe("page");
  });

  it("retains combining marks so distinct titles do not collapse", () => {
    expect(slugifyWikiSegment("किताब")).toBe("किताब");
    expect(slugifyWikiSegment("कुतुब")).toBe("कुतुब");
    expect(slugifyWikiSegment("कीताब")).toBe("कीताब");
  });

  it("caps long Unicode slugs to a safe filename byte length", () => {
    const title = "漢".repeat(90);
    const slug = slugifyWikiSegment(title);

    expect(slug.endsWith(`-${createHash("sha1").update(title).digest("hex").slice(0, 12)}`)).toBe(
      true,
    );
    expect(Buffer.byteLength(slug)).toBeLessThanOrEqual(240);
    expect(slugifyWikiSegment(title)).toBe(slug);
  });

  it("caps composed wiki page filenames to a safe path-component length", () => {
    const stem = `bridge-${"漢".repeat(45)}-${"語".repeat(45)}`;
    const fileName = createWikiPageFilename(stem);

    expect(fileName.endsWith(".md")).toBe(true);
    expect(Buffer.byteLength(`.${fileName}.fallback.tmp`)).toBeLessThanOrEqual(255);
    expect(createWikiPageFilename(stem)).toBe(fileName);
  });
});

describe("inferWikiPageKind", () => {
  it("returns null when no pageGroups given and path not matched by default", () => {
    expect(inferWikiPageKind("unknown/file.md")).toBeNull();
  });

  it("returns null for reports/ paths when no pageGroups given", () => {
    // reports/ is no longer hardcoded — pageGroups must include a report group
    expect(inferWikiPageKind("reports/index.md")).toBeNull();
    expect(inferWikiPageKind("reports/lint.md")).toBeNull();
  });

  it("returns 'report' for report dir paths when pageGroups includes report kind", () => {
    const pageGroups: import("./config.js").WikiPageGroup[] = [
      { kind: "source", dir: "sources", heading: "Sources" },
      { kind: "report", dir: "reports", heading: "Reports" },
    ];
    expect(inferWikiPageKind("reports/index.md", pageGroups)).toBe("report");
    expect(inferWikiPageKind("reports/lint.md", pageGroups)).toBe("report");
  });

  it("returns 'report' for Chinese report dir when pageGroups includes it", () => {
    const pageGroups: import("./config.js").WikiPageGroup[] = [
      { kind: "report", dir: "报告", heading: "报告" },
    ];
    expect(inferWikiPageKind("报告/index.md", pageGroups)).toBe("report");
    expect(inferWikiPageKind("报告/lint.md", pageGroups)).toBe("report");
  });

  it("matches paths against provided pageGroups", () => {
    expect(inferWikiPageKind("sources/alpha.md", DEFAULT_PAGE_GROUPS)).toBe("source");
    expect(inferWikiPageKind("entities/brad.md", DEFAULT_PAGE_GROUPS)).toBe("entity");
    expect(inferWikiPageKind("concepts/sre.md", DEFAULT_PAGE_GROUPS)).toBe("concept");
    expect(inferWikiPageKind("syntheses/report.md", DEFAULT_PAGE_GROUPS)).toBe("synthesis");
  });

  it("matches nested subdirectory paths", () => {
    expect(inferWikiPageKind("entities/tech/ai/llm.md", DEFAULT_PAGE_GROUPS)).toBe("entity");
    expect(inferWikiPageKind("sources/web/zhihu-article.md", DEFAULT_PAGE_GROUPS)).toBe("source");
  });

  it("matches root-level files when pageGroups has dir '.'", () => {
    const pageGroups: WikiPageGroup[] = [
      { kind: "source", dir: ".", heading: "Root" },
    ];
    expect(inferWikiPageKind("index.md", pageGroups)).toBe("source");
    expect(inferWikiPageKind("readme.md", pageGroups)).toBe("source");
    // Nested files should not match "."
    expect(inferWikiPageKind("sources/file.md", pageGroups)).toBeNull();
  });

  it("sorts by dir length so longer prefix wins", () => {
    const pageGroups: WikiPageGroup[] = [
      { kind: "source", dir: "data/raw", heading: "Raw" },
      { kind: "concept", dir: "data", heading: "Data" },
    ];
    expect(inferWikiPageKind("data/raw/sensor.csv.md", pageGroups)).toBe("source");
    expect(inferWikiPageKind("data/processed.csv.md", pageGroups)).toBe("concept");
  });

  it("matches custom dirs like views/ or templates/", () => {
    const pageGroups: WikiPageGroup[] = [
      { kind: "synthesis", dir: "views", heading: "Views" },
      { kind: "synthesis", dir: "templates", heading: "Templates" },
    ];
    expect(inferWikiPageKind("views/dashboard.md", pageGroups)).toBe("synthesis");
    expect(inferWikiPageKind("templates/standard.md", pageGroups)).toBe("synthesis");
  });

  it("returns 'report' for reports/ when pageGroups includes report kind", () => {
    const groupsWithReport = [...DEFAULT_PAGE_GROUPS, { kind: "report" as const, dir: "reports", heading: "Reports" }];
    expect(inferWikiPageKind("reports/lint.md", groupsWithReport)).toBe("report");
  });

  it("returns null for reports/ when pageGroups does not include report kind", () => {
    expect(inferWikiPageKind("reports/lint.md", DEFAULT_PAGE_GROUPS)).toBeNull();
  });
});

describe("toWikiPageSummary", () => {
  it("returns null for unknown paths without pageGroups", () => {
    const result = toWikiPageSummary({
      absolutePath: "/tmp/wiki/unknown/file.md",
      relativePath: "unknown/file.md",
      raw: renderWikiMarkdown({ frontmatter: { title: "Test" }, body: "# Test\n" }),
    });
    expect(result).toBeNull();
  });

  it("classifies files using custom pageGroups (views/)", () => {
    const raw = renderWikiMarkdown({ frontmatter: { title: "My View" }, body: "# My View\n" });

    const summary = toWikiPageSummary({
      absolutePath: "/tmp/wiki/views/dashboard.md",
      relativePath: "views/dashboard.md",
      raw,
      pageGroups: [
        { kind: "synthesis", dir: "views", heading: "Views" },
      ],
    });

    expect(summary).not.toBeNull();
    expect(summary!.kind).toBe("synthesis");
    expect(summary!.title).toBe("My View");
    expect(summary!.relativePath).toBe("views/dashboard.md");
  });

  it("classifies root-level files with pageGroups dir '.'", () => {
    const raw = renderWikiMarkdown({ frontmatter: { title: "Root Doc" }, body: "# Root Doc\n" });

    const summary = toWikiPageSummary({
      absolutePath: "/tmp/wiki/api-guide.md",
      relativePath: "api-guide.md",
      raw,
      pageGroups: [
        { kind: "source", dir: ".", heading: "Root" },
      ],
    });

    expect(summary).not.toBeNull();
    expect(summary!.kind).toBe("source");
    expect(summary!.title).toBe("Root Doc");
  });

  it("uses pageGroups order to match nested subdirectories", () => {
    const raw = renderWikiMarkdown({ frontmatter: { title: "AI Entity" }, body: "# AI Entity\n" });

    const summary = toWikiPageSummary({
      absolutePath: "/tmp/wiki/entities/tech/ai/llm.md",
      relativePath: "entities/tech/ai/llm.md",
      raw,
      pageGroups: DEFAULT_PAGE_GROUPS,
    });

    expect(summary).not.toBeNull();
    expect(summary!.kind).toBe("entity");
  });

  it("prefers longer pageGroups dir match over shorter one", () => {
    const raw = renderWikiMarkdown({ frontmatter: { title: "Deep Entity" }, body: "# Deep Entity\n" });

    const summary = toWikiPageSummary({
      absolutePath: "/tmp/wiki/entities/tech/deep.md",
      relativePath: "entities/tech/deep.md",
      raw,
      pageGroups: [
        { kind: "source", dir: "entities/tech", heading: "Tech Sources" },
        { kind: "entity", dir: "entities", heading: "Entities" },
      ],
    });

    // Should match the longer "entities/tech" prefix (source), not just "entities" (entity)
    expect(summary).not.toBeNull();
    expect(summary!.kind).toBe("source");
  });

  it("normalizes agent-facing people wiki metadata", () => {
    const raw = renderWikiMarkdown({
      frontmatter: {
        pageType: "entity",
        entityType: "person",
        id: "entity.brad",
        title: "Brad Groux",
        canonicalId: "maintainer.brad-groux",
        aliases: ["brad", "bgroux"],
        privacyTier: "local-private",
        bestUsedFor: ["Microsoft ecosystem routing"],
        notEnoughFor: ["legal approval"],
        lastRefreshedAt: "2026-04-29T00:00:00.000Z",
        personCard: {
          handles: ["@bgroux"],
          socials: ["https://x.example/bgroux"],
          email: "brad@example.com",
          timezone: "America/Chicago",
          lane: "Microsoft Teams",
          askFor: ["Teams and Azure questions"],
          avoidAskingFor: ["unrelated billing"],
          confidence: 0.8,
          privacyTier: "confirm-before-use",
          lastRefreshedAt: "2026-04-28T00:00:00.000Z",
        },
        relationships: [
          {
            targetId: "entity.alice",
            targetTitle: "Alice",
            kind: "collaborates-with",
            weight: 0.7,
            confidence: 0.6,
            evidenceKind: "discrawl-stat",
            privacyTier: "local-private",
          },
        ],
        claims: [
          {
            id: "claim.brad.teams",
            text: "Brad is useful for Microsoft Teams routing.",
            confidence: 0.9,
            evidence: [
              {
                kind: "maintainer-whois",
                sourceId: "source.maintainers",
                confidence: 0.8,
                privacyTier: "local-private",
              },
            ],
          },
        ],
      },
      body: "# Brad Groux\n",
    });

    const summary = toWikiPageSummary({
      absolutePath: "/tmp/wiki/entities/brad.md",
      relativePath: "entities/brad.md",
      raw,
      pageGroups: DEFAULT_PAGE_GROUPS,
    });

    expect(summary).toEqual(
      expect.objectContaining({
        entityType: "person",
        canonicalId: "maintainer.brad-groux",
        aliases: ["brad", "bgroux"],
        privacyTier: "local-private",
        bestUsedFor: ["Microsoft ecosystem routing"],
        notEnoughFor: ["legal approval"],
        lastRefreshedAt: "2026-04-29T00:00:00.000Z",
        personCard: expect.objectContaining({
          handles: ["@bgroux"],
          emails: ["brad@example.com"],
          lane: "Microsoft Teams",
          privacyTier: "confirm-before-use",
        }),
        relationships: [
          expect.objectContaining({
            targetId: "entity.alice",
            kind: "collaborates-with",
            evidenceKind: "discrawl-stat",
          }),
        ],
        claims: [
          expect.objectContaining({
            id: "claim.brad.teams",
            evidence: [
              expect.objectContaining({
                kind: "maintainer-whois",
                privacyTier: "local-private",
              }),
            ],
          }),
        ],
      }),
    );
  });
});
