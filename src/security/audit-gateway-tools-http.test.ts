import { describe, expect, it } from "vitest";
import type { OpenClawConfig } from "../config/config.js";
import { collectGatewayConfigFindings } from "./audit-gateway-config.js";

function hasFinding(
  findings: ReturnType<typeof collectGatewayConfigFindings>,
  checkId: string,
  severity?: "warn" | "critical",
) {
  return findings.some(
    (finding) => finding.checkId === checkId && (severity == null || finding.severity === severity),
  );
}

describe("security audit gateway HTTP tool findings", () => {
  it.each([
    {
      name: "loopback bind",
      cfg: {
        gateway: {
          bind: "loopback",
          auth: { token: "secret" },
          tools: { allow: ["sessions_spawn"] },
        },
      } satisfies OpenClawConfig,
      expectedSeverity: "warn" as const,
    },
    {
      name: "non-loopback bind",
      cfg: {
        gateway: {
          bind: "lan",
          auth: { token: "secret" },
          tools: { allow: ["sessions_spawn", "gateway"] },
        },
      } satisfies OpenClawConfig,
      expectedSeverity: "critical" as const,
    },
    {
      name: "newly denied exec override",
      cfg: {
        gateway: {
          bind: "lan",
          auth: { token: "secret" },
          tools: { allow: ["exec"] },
        },
      } satisfies OpenClawConfig,
      expectedSeverity: "critical" as const,
    },
  ])(
    "scores dangerous gateway.tools.allow over HTTP by exposure: $name",
    ({ cfg, expectedSeverity }) => {
      const findings = collectGatewayConfigFindings(cfg, cfg, {});
      expect(
        hasFinding(findings, "gateway.tools_invoke_http.dangerous_allow", expectedSeverity),
      ).toBe(true);
    },
  );

  it("emits a host-file-read finding (not RCE) when only `read` is opted in", () => {
    // `read` is on DEFAULT_GATEWAY_HTTP_TOOL_DENY but its exposure shape is
    // host filesystem read access (information disclosure), not RCE / session
    // orchestration. The audit must emit a separate finding with appropriately
    // scoped wording.
    const cfg = {
      gateway: {
        bind: "loopback",
        auth: { token: "secret" },
        tools: { allow: ["read"] },
      },
    } satisfies OpenClawConfig;
    const findings = collectGatewayConfigFindings(cfg, cfg, {});
    expect(hasFinding(findings, "gateway.tools_invoke_http.host_read_allow", "warn")).toBe(true);
    // And the RCE-flavored finding should NOT fire for a read-only opt-in.
    expect(hasFinding(findings, "gateway.tools_invoke_http.dangerous_allow")).toBe(false);
  });
});
