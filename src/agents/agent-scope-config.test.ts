import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  listAgentIds,
  resolveAgentWorkspaceDir,
  resolveDefaultAgentDir,
  resolveDefaultAgentId,
} from "./agent-scope-config.js";

describe("resolveDefaultAgentId", () => {
  it("returns config defaultAgentId when agents.list is absent", () => {
    const result = resolveDefaultAgentId({ agents: { defaultAgentId: "ops" } }, {});
    expect(result).toBe("ops");
  });

  it("returns config defaultAgentId when agents.list is empty", () => {
    const result = resolveDefaultAgentId({ agents: { list: [], defaultAgentId: "ops" } }, {});
    expect(result).toBe("ops");
  });

  it("returns first agents.list entry when list is populated, ignoring defaultAgentId", () => {
    const result = resolveDefaultAgentId(
      {
        agents: {
          list: [{ id: "alpha" }, { id: "beta" }],
          defaultAgentId: "ops",
        },
      },
      {},
    );
    expect(result).toBe("alpha");
  });

  it("returns agents.list default:true entry over first when present", () => {
    const result = resolveDefaultAgentId(
      {
        agents: {
          list: [{ id: "alpha" }, { id: "beta", default: true }],
          defaultAgentId: "ops",
        },
      },
      {},
    );
    expect(result).toBe("beta");
  });

  it("falls back to OPENCLAW_DEFAULT_AGENT_ID env when no agents and no config defaultAgentId", () => {
    const result = resolveDefaultAgentId({}, { OPENCLAW_DEFAULT_AGENT_ID: "envagent" });
    expect(result).toBe("envagent");
  });

  it("config defaultAgentId takes precedence over OPENCLAW_DEFAULT_AGENT_ID env", () => {
    const result = resolveDefaultAgentId(
      { agents: { defaultAgentId: "cfgagent" } },
      { OPENCLAW_DEFAULT_AGENT_ID: "envagent" },
    );
    expect(result).toBe("cfgagent");
  });

  it("returns DEFAULT_AGENT_ID when nothing is configured", () => {
    const result = resolveDefaultAgentId({}, {});
    expect(result).toBe("main");
  });
});

describe("resolveDefaultAgentDir", () => {
  it("routes the config default agent through the matching agent directory", () => {
    const result = resolveDefaultAgentDir(
      { agents: { defaultAgentId: "Ops Team" } },
      { OPENCLAW_STATE_DIR: "/tmp/openclaw" },
    );

    expect(result).toBe(path.join("/tmp/openclaw", "agents", "ops-team", "agent"));
  });

  it("uses an explicit agentDir from the default list entry", () => {
    const result = resolveDefaultAgentDir(
      {
        agents: {
          list: [{ id: "ops", default: true, agentDir: "~/ops-agent" }],
        },
      },
      { HOME: "/home/tester", OPENCLAW_STATE_DIR: "/tmp/openclaw" },
    );

    expect(result).toBe(path.join("/home/tester", "ops-agent"));
  });
});

describe("listAgentIds", () => {
  it("returns config defaultAgentId when agents.list is absent", () => {
    expect(listAgentIds({ agents: { defaultAgentId: "ops" } })).toEqual(["ops"]);
  });

  it("returns env default agent id when no agents are configured", () => {
    const previous = process.env.OPENCLAW_DEFAULT_AGENT_ID;
    process.env.OPENCLAW_DEFAULT_AGENT_ID = "envagent";
    try {
      expect(listAgentIds({})).toEqual(["envagent"]);
    } finally {
      if (previous === undefined) {
        delete process.env.OPENCLAW_DEFAULT_AGENT_ID;
      } else {
        process.env.OPENCLAW_DEFAULT_AGENT_ID = previous;
      }
    }
  });
});

describe("resolveAgentWorkspaceDir env-threading", () => {
  it("routes env-only default agent to default workspace path, not a per-agent subdirectory", () => {
    // When OPENCLAW_DEFAULT_AGENT_ID=envagent and no agents.list is configured,
    // resolveAgentWorkspaceDir should treat that agent as the default and return
    // the default workspace dir (not a workspace-envagent subdirectory).
    const env = {
      OPENCLAW_DEFAULT_AGENT_ID: "envagent",
      OPENCLAW_STATE_DIR: "/tmp/openclaw",
      HOME: "/home/tester",
    };
    const result = resolveAgentWorkspaceDir({}, "envagent", env);
    // Should be the default workspace path, not path.join(..., "workspace-envagent")
    expect(result).not.toContain("workspace-envagent");
  });

  it("routes non-default agent to per-agent workspace subdir when env default is different", () => {
    const env = {
      OPENCLAW_DEFAULT_AGENT_ID: "mainagent",
      OPENCLAW_STATE_DIR: "/tmp/openclaw",
      HOME: "/home/tester",
    };
    const result = resolveAgentWorkspaceDir({}, "otheragent", env);
    expect(result).toContain("workspace-otheragent");
  });

  it("respects configured per-agent workspace, ignoring default resolution", () => {
    const result = resolveAgentWorkspaceDir(
      {
        agents: {
          list: [{ id: "envagent", workspace: "~/custom-workspace" }],
        },
      },
      "envagent",
      { HOME: "/home/tester", OPENCLAW_DEFAULT_AGENT_ID: "envagent" },
    );
    expect(result).toBe(path.join("/home/tester", "custom-workspace"));
  });

  it("env default agent with agents.defaults.workspace configured uses default workspace path", () => {
    const env = {
      OPENCLAW_DEFAULT_AGENT_ID: "envagent",
      HOME: "/home/tester",
    };
    const result = resolveAgentWorkspaceDir(
      { agents: { defaults: { workspace: "~/shared-workspace" } } },
      "envagent",
      env,
    );
    expect(result).toBe(path.join("/home/tester", "shared-workspace"));
  });
});
