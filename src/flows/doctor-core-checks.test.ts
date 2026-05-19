import { promises as fs } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { OpenClawConfig } from "../config/types.openclaw.js";
import {
  CORE_HEALTH_CHECKS,
  registerCoreHealthChecks,
  resetCoreHealthChecksForTest,
} from "./doctor-core-checks.js";
import { doctorHealthConversionRules } from "./doctor-health-conversion-plan.js";
import {
  clearHealthChecksForTest,
  listHealthChecks,
  registerHealthCheck,
} from "./health-check-registry.js";

describe("registerCoreHealthChecks", () => {
  let tmp: string | undefined;

  beforeEach(() => {
    clearHealthChecksForTest();
    resetCoreHealthChecksForTest();
  });

  afterEach(async () => {
    if (tmp !== undefined) {
      await fs.rm(tmp, { recursive: true, force: true });
      tmp = undefined;
    }
  });

  it("registers the built-in health checks once", () => {
    registerCoreHealthChecks();
    registerCoreHealthChecks();

    expect(listHealthChecks().map((check) => check.id)).toEqual(
      CORE_HEALTH_CHECKS.map((check) => check.id),
    );
  });

  it("can retry after a duplicate registration failure is cleared", () => {
    registerHealthCheck({
      id: "core/doctor/gateway-config",
      kind: "core",
      description: "duplicate",
      async detect() {
        return [];
      },
    });

    expect(() => registerCoreHealthChecks()).toThrow("health check already registered");

    clearHealthChecksForTest();
    registerCoreHealthChecks();

    expect(listHealthChecks()).toHaveLength(CORE_HEALTH_CHECKS.length);
  });

  it("registers only implemented core health targets from the doctor conversion inventory", () => {
    registerCoreHealthChecks();

    const registeredIds = new Set(listHealthChecks().map((check) => check.id));
    const coreTargets = new Set<string>(
      doctorHealthConversionRules.flatMap((rule) =>
        rule.target.filter((target) => target.startsWith("core/doctor/")),
      ),
    );
    const plannedOnlyTargets = [
      "core/doctor/auth-profiles/keychain",
      "core/doctor/session-locks",
      "core/doctor/gateway-daemon",
    ];

    for (const id of CORE_HEALTH_CHECKS.map((check) => check.id)) {
      if (id === "core/doctor/browser-clawd-profile-residue") {
        continue;
      }
      expect(coreTargets.has(id)).toBe(true);
    }
    for (const id of plannedOnlyTargets) {
      expect(registeredIds.has(id)).toBe(false);
    }
    expect(
      CORE_HEALTH_CHECKS.some((check) =>
        check.description.endsWith("represented in the health registry."),
      ),
    ).toBe(false);
  });

  it("shows the repair-capable health check shape with skills readiness", async () => {
    tmp = await fs.mkdtemp(join(tmpdir(), "openclaw-health-skills-"));
    const skillDir = join(tmp, "skills", "missing-tool");
    await fs.mkdir(skillDir, { recursive: true });
    await fs.writeFile(
      join(skillDir, "SKILL.md"),
      `---
name: missing-tool
description: Missing tool
metadata: '{"openclaw":{"requires":{"bins":["openclaw-test-missing-skill-bin"]}}}'
---

# Missing tool
`,
      "utf-8",
    );
    const cfg: OpenClawConfig = {
      agents: {
        defaults: {
          workspace: tmp,
          skills: ["missing-tool"],
        },
      },
    };
    const check = CORE_HEALTH_CHECKS.find((entry) => entry.id === "core/doctor/skills-readiness");

    expect(check?.repair).toBeTypeOf("function");

    const findings = await check?.detect({
      mode: "lint",
      runtime: { log() {}, error() {}, exit() {} },
      cfg,
      cwd: tmp,
    });
    expect(findings).toContainEqual(
      expect.objectContaining({
        checkId: "core/doctor/skills-readiness",
        severity: "warning",
        path: "skills.entries.missing-tool.enabled",
      }),
    );
    await expect(
      check?.detect(
        {
          mode: "fix",
          runtime: { log() {}, error() {}, exit() {} },
          cfg,
          cwd: tmp,
        },
        { paths: ["skills.entries.other-tool.enabled"] },
      ),
    ).resolves.toEqual([]);
    await expect(
      check?.detect(
        {
          mode: "fix",
          runtime: { log() {}, error() {}, exit() {} },
          cfg,
          cwd: tmp,
        },
        { paths: ["skills.entries.missing-tool.enabled"] },
      ),
    ).resolves.toContainEqual(
      expect.objectContaining({
        path: "skills.entries.missing-tool.enabled",
      }),
    );

    const repaired = await check?.repair?.(
      {
        mode: "fix",
        runtime: { log() {}, error() {}, exit() {} },
        cfg,
        cwd: tmp,
      },
      findings ?? [],
    );
    expect(repaired?.config?.skills?.entries?.["missing-tool"]).toEqual({ enabled: false });
    expect(repaired?.changes).toContain("Disabled unavailable skill missing-tool.");
    expect(repaired?.effects).toContainEqual(
      expect.objectContaining({
        kind: "config",
        action: "disable-skill",
        target: "skills.entries.missing-tool.enabled",
      }),
    );
  });

  it("converts security doctor warnings into health findings", async () => {
    const check = CORE_HEALTH_CHECKS.find((entry) => entry.id === "core/doctor/security");

    const findings = await check?.detect({
      mode: "lint",
      runtime: { log() {}, error() {}, exit() {} },
      cfg: {
        gateway: {
          bind: "lan",
          auth: {
            mode: "none",
          },
        },
      },
    });

    expect(findings).toContainEqual(
      expect.objectContaining({
        checkId: "core/doctor/security",
        severity: "error",
        message: expect.stringContaining("Gateway bound"),
      }),
    );
  });

  it("skips gateway auth warning when SecretRef-managed token resolves in lint checks", async () => {
    const check = CORE_HEALTH_CHECKS.find((entry) => entry.id === "core/doctor/gateway-auth");
    const previousToken = process.env.OPENCLAW_TEST_GATEWAY_TOKEN;
    process.env.OPENCLAW_TEST_GATEWAY_TOKEN = "resolved-test-token";
    try {
      const findings = await check?.detect({
        mode: "lint",
        runtime: { log() {}, error() {}, exit() {} },
        cfg: {
          gateway: {
            mode: "local",
            auth: {
              mode: "token",
              token: {
                source: "env",
                provider: "default",
                id: "OPENCLAW_TEST_GATEWAY_TOKEN",
              },
            },
          },
          secrets: {
            providers: {
              default: { source: "env" },
            },
          },
        },
        cwd: tmp,
      });

      expect(findings).toEqual([]);
    } finally {
      if (previousToken === undefined) {
        delete process.env.OPENCLAW_TEST_GATEWAY_TOKEN;
      } else {
        process.env.OPENCLAW_TEST_GATEWAY_TOKEN = previousToken;
      }
    }
  });

  it("does not execute or warn for valid exec SecretRefs during default gateway auth lint checks", async () => {
    tmp = await fs.mkdtemp(join(tmpdir(), "openclaw-health-exec-ref-"));
    const markerPath = join(tmp, "exec-ran");
    const check = CORE_HEALTH_CHECKS.find((entry) => entry.id === "core/doctor/gateway-auth");

    const findings = await check?.detect({
      mode: "lint",
      runtime: { log() {}, error() {}, exit() {} },
      cfg: {
        gateway: {
          mode: "local",
          auth: {
            mode: "token",
            token: {
              source: "exec",
              provider: "default",
              id: "value",
            },
          },
        },
        secrets: {
          providers: {
            default: {
              source: "exec",
              command: "/bin/sh",
              args: ["-c", `cat >/dev/null; printf executed > ${JSON.stringify(markerPath)}`],
              jsonOnly: false,
              allowInsecurePath: true,
            },
          },
        },
      },
      cwd: tmp,
    });

    expect(findings).toEqual([]);
    await expect(fs.readFile(markerPath, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("executes exec SecretRefs when gateway auth lint explicitly allows exec checks", async () => {
    tmp = await fs.mkdtemp(join(tmpdir(), "openclaw-health-exec-ref-"));
    const markerPath = join(tmp, "exec-ran");
    const check = CORE_HEALTH_CHECKS.find((entry) => entry.id === "core/doctor/gateway-auth");

    const findings = await check?.detect({
      mode: "lint",
      runtime: { log() {}, error() {}, exit() {} },
      cfg: {
        gateway: {
          mode: "local",
          auth: {
            mode: "token",
            token: {
              source: "exec",
              provider: "default",
              id: "value",
            },
          },
        },
        secrets: {
          providers: {
            default: {
              source: "exec",
              command: "/bin/sh",
              args: [
                "-c",
                `cat >/dev/null; printf executed > ${JSON.stringify(markerPath)}; printf resolved-token`,
              ],
              jsonOnly: false,
              allowInsecurePath: true,
            },
          },
        },
      },
      cwd: tmp,
      allowExecSecretRefs: true,
    });

    expect(findings).toEqual([]);
    await expect(fs.readFile(markerPath, "utf8")).resolves.toBe("executed");
  });

  it("reports exec SecretRef failures when gateway auth lint explicitly allows exec checks", async () => {
    const check = CORE_HEALTH_CHECKS.find((entry) => entry.id === "core/doctor/gateway-auth");

    const findings = await check?.detect({
      mode: "lint",
      runtime: { log() {}, error() {}, exit() {} },
      cfg: {
        gateway: {
          mode: "local",
          auth: {
            mode: "token",
            token: {
              source: "exec",
              provider: "default",
              id: "value",
            },
          },
        },
        secrets: {
          providers: {
            default: {
              source: "exec",
              command: "/bin/sh",
              args: ["-c", "cat >/dev/null; exit 12"],
              jsonOnly: false,
              allowInsecurePath: true,
            },
          },
        },
      },
      allowExecSecretRefs: true,
    });

    expect(findings).toContainEqual(
      expect.objectContaining({
        checkId: "core/doctor/gateway-auth",
        severity: "warning",
        message: expect.stringContaining("Gateway token SecretRef could not be resolved:"),
        fixHint:
          "Run `openclaw doctor --allow-exec` to verify exec SecretRefs during doctor, or `openclaw secrets audit --allow-exec` to audit all exec SecretRefs.",
      }),
    );
  });

  it("converts workspace suggestions into info findings", async () => {
    tmp = await fs.mkdtemp(join(tmpdir(), "openclaw-health-workspace-"));
    const check = CORE_HEALTH_CHECKS.find(
      (entry) => entry.id === "core/doctor/workspace-suggestions",
    );

    const findings = await check?.detect({
      mode: "lint",
      runtime: { log() {}, error() {}, exit() {} },
      cfg: {
        agents: {
          defaults: {
            workspace: tmp,
          },
        },
      },
      cwd: tmp,
    });

    expect(findings).toContainEqual(
      expect.objectContaining({
        checkId: "core/doctor/workspace-suggestions",
        severity: "info",
        message: "Tip: back up the workspace in a private git repo (GitHub or GitLab).",
      }),
    );
    expect(findings).toContainEqual(
      expect.objectContaining({
        checkId: "core/doctor/workspace-suggestions",
        severity: "info",
        message: "Memory system not found in workspace.",
      }),
    );
  });
});
