import type { Command } from "commander";
import {
  resolveAgentIdByWorkspacePath,
  resolveAgentWorkspaceDir,
  resolveDefaultAgentId,
} from "../agents/agent-scope.js";
import {
  installSkillFromClawHub,
  readTrackedClawHubSkillSlugs,
  searchSkillsFromClawHub,
  updateSkillsFromClawHub,
  verifySkillFromClawHub,
} from "../agents/skills-clawhub.js";
import {
  installSkillFromSource,
  isSkillSourceInstallSpec,
} from "../agents/skills-source-install.js";
import { getRuntimeConfig } from "../config/config.js";
import type { ClawHubSkillTrustCard } from "../infra/clawhub.js";
import { defaultRuntime } from "../runtime.js";
import { normalizeOptionalString } from "../shared/string-coerce.js";
import { formatDocsLink } from "../terminal/links.js";
import { theme } from "../terminal/theme.js";
import { CONFIG_DIR } from "../utils.js";
import { resolveOptionFromCommand } from "./cli-utils.js";
import { formatSkillInfo, formatSkillsCheck, formatSkillsList } from "./skills-cli.format.js";

export type {
  SkillInfoOptions,
  SkillsCheckOptions,
  SkillsListOptions,
} from "./skills-cli.format.js";
export { formatSkillInfo, formatSkillsCheck, formatSkillsList } from "./skills-cli.format.js";

type SkillStatusReport = Awaited<
  ReturnType<(typeof import("../agents/skills-status.js"))["buildWorkspaceSkillStatus"]>
>;

type ResolveSkillsWorkspaceOptions = {
  agentId?: string;
  cwd?: string;
};

function resolveSkillsWorkspace(options?: ResolveSkillsWorkspaceOptions): {
  config: ReturnType<typeof getRuntimeConfig>;
  workspaceDir: string;
  agentId: string;
} {
  const config = getRuntimeConfig();
  const explicitAgentId = normalizeOptionalString(options?.agentId);
  const inferredAgentId = explicitAgentId
    ? undefined
    : resolveAgentIdByWorkspacePath(config, options?.cwd ?? process.cwd());
  const agentId = explicitAgentId ?? inferredAgentId ?? resolveDefaultAgentId(config);
  return {
    config,
    agentId,
    workspaceDir: resolveAgentWorkspaceDir(config, agentId),
  };
}

function resolveAgentOption(
  command: Command | undefined,
  opts?: { agent?: string },
): string | undefined {
  return resolveOptionFromCommand<string>(command, "agent") ?? opts?.agent;
}

async function loadSkillsStatusReport(
  options?: ResolveSkillsWorkspaceOptions,
): Promise<SkillStatusReport> {
  const { config, workspaceDir, agentId } = resolveSkillsWorkspace(options);
  const { buildWorkspaceSkillStatus } = await import("../agents/skills-status.js");
  return buildWorkspaceSkillStatus(workspaceDir, { config, agentId });
}

async function runSkillsAction(
  render: (report: SkillStatusReport) => string,
  options?: ResolveSkillsWorkspaceOptions,
): Promise<void> {
  try {
    const report = await loadSkillsStatusReport(options);
    defaultRuntime.writeStdout(render(report));
    defaultRuntime.exit(0);
  } catch (err) {
    defaultRuntime.error(String(err));
    defaultRuntime.exit(1);
  }
}

function resolveActiveWorkspaceDir(options?: ResolveSkillsWorkspaceOptions): string {
  return resolveSkillsWorkspace(options).workspaceDir;
}

function resolveClawHubTargetWorkspaceDir(
  command: Command | undefined,
  opts: { agent?: string; global?: boolean },
): string | undefined {
  const agentId = resolveAgentOption(command, opts);
  if (opts.global && normalizeOptionalString(agentId)) {
    defaultRuntime.error("Use either --global or --agent, not both.");
    defaultRuntime.exit(1);
    return undefined;
  }
  if (opts.global) {
    return CONFIG_DIR;
  }
  return resolveActiveWorkspaceDir({ agentId });
}

function trustAuditStatusLabel(status: string | undefined): string {
  const label = (status ?? "unknown").toUpperCase();
  if (status === "pass") {
    return theme.success(label);
  }
  if (status === "review" || status === "pending") {
    return theme.warn(label);
  }
  if (status === "malicious" || status === "error") {
    return theme.error(label);
  }
  return label;
}

function trustCardString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function trustCardStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.trim().length > 0)
    : [];
}

function formatTrustCardSource(source: ClawHubSkillTrustCard["source"]): string | undefined {
  if (!source) {
    return undefined;
  }
  const repo = trustCardString(source.repo);
  const commit = trustCardString(source.commit);
  const sourcePath = trustCardString(source.path);
  const url = trustCardString(source.url);
  if (repo && commit && sourcePath) {
    return `${repo}@${commit.slice(0, 12)} ${sourcePath}`;
  }
  return url;
}

function formatTrustCardRequirementList(label: string, value: unknown): string | undefined {
  const items = trustCardStringArray(value);
  return items.length ? `${label}=${items.join(",")}` : undefined;
}

function formatTrustCardRequires(
  requires: Record<string, unknown> | null | undefined,
): string | undefined {
  if (!requires || typeof requires !== "object") {
    return undefined;
  }
  const record = requires;
  const parts = [
    formatTrustCardRequirementList("env", record.env),
    formatTrustCardRequirementList("bins", record.bins),
    formatTrustCardRequirementList("anyBins", record.anyBins),
    formatTrustCardRequirementList("config", record.config),
  ].filter((part): part is string => Boolean(part));
  return parts.length ? parts.join("; ") : undefined;
}

function printSkillTrustVerification(params: {
  slug: string;
  resolvedFrom: "installed" | "version" | "tag" | "latest";
  registry: string;
  trustCard: ClawHubSkillTrustCard;
}): string | undefined {
  const card = params.trustCard;
  const subjectSlug = trustCardString(card.subject?.slug) ?? params.slug;
  const version = trustCardString(card.subject?.version) ?? "?";
  const displayName = trustCardString(card.subject?.displayName);
  const auditStatus = trustCardString(card.audit?.status);
  const signatureStatus = trustCardString(card.signature?.status) ?? "unknown";
  const fingerprint = trustCardString(card.artifact?.fingerprint);
  const publisher =
    trustCardString(card.publisher?.handle) ?? trustCardString(card.publisher?.displayName);
  const capabilities = trustCardStringArray(card.capabilities?.tags);
  const source = formatTrustCardSource(card.source);
  const requires = formatTrustCardRequires(card.capabilities?.requires);

  defaultRuntime.log(`${subjectSlug}@${version} trust`);
  if (displayName) {
    defaultRuntime.log(`Name: ${displayName}`);
  }
  defaultRuntime.log(`Registry: ${params.registry}`);
  defaultRuntime.log(`Resolved: ${params.resolvedFrom}`);
  if (publisher) {
    defaultRuntime.log(`Publisher: ${publisher}`);
  }
  defaultRuntime.log(`Audit: ${trustAuditStatusLabel(auditStatus)}`);
  if (card.audit?.summary) {
    defaultRuntime.log(`Audit Summary: ${card.audit.summary}`);
  }
  if (card.audit?.reasonCodes?.length) {
    defaultRuntime.log(`Audit Reasons: ${card.audit.reasonCodes.join(", ")}`);
  }
  defaultRuntime.log(`Signature: ${signatureStatus}`);
  if (fingerprint) {
    defaultRuntime.log(`Fingerprint: ${fingerprint}`);
  }
  defaultRuntime.log(`Files: ${card.artifact?.files?.length ?? 0}`);
  if (source) {
    defaultRuntime.log(`Source: ${source}`);
  }
  if (capabilities.length) {
    defaultRuntime.log(`Capabilities: ${capabilities.join(", ")}`);
  }
  if (requires) {
    defaultRuntime.log(`Requires: ${requires}`);
  }
  return auditStatus;
}

/**
 * Register the skills CLI commands
 */
export function registerSkillsCli(program: Command) {
  const skills = program
    .command("skills")
    .description("List and inspect available skills")
    .option("--agent <id>", "Target agent workspace (defaults to cwd-inferred, then default agent)")
    .addHelpText(
      "after",
      () =>
        `\n${theme.muted("Docs:")} ${formatDocsLink("/cli/skills", "docs.openclaw.ai/cli/skills")}\n`,
    );

  skills
    .command("search")
    .description("Search ClawHub skills")
    .argument("[query...]", "Optional search query")
    .option("--limit <n>", "Max results", (value) => Number.parseInt(value, 10))
    .option("--json", "Output as JSON", false)
    .action(async (queryParts: string[], opts: { limit?: number; json?: boolean }) => {
      try {
        const results = await searchSkillsFromClawHub({
          query: normalizeOptionalString(queryParts.join(" ")),
          limit: opts.limit,
        });
        if (opts.json) {
          defaultRuntime.writeJson({ results });
          return;
        }
        if (results.length === 0) {
          defaultRuntime.log("No ClawHub skills found.");
          return;
        }
        for (const entry of results) {
          const version = entry.version ? ` v${entry.version}` : "";
          const summary = entry.summary ? `  ${entry.summary}` : "";
          defaultRuntime.log(`${entry.slug}${version}  ${entry.displayName}${summary}`);
        }
      } catch (err) {
        defaultRuntime.error(String(err));
        defaultRuntime.exit(1);
      }
    });

  skills
    .command("install")
    .description("Install a skill from ClawHub, git, or a local directory")
    .argument("<slug>", "ClawHub skill slug, git:<repo>, or local skill directory")
    .option("--version <version>", "Install a specific version")
    .option("--force", "Overwrite an existing workspace skill", false)
    .option("--global", "Install into the shared managed skills directory", false)
    .option("--agent <id>", "Target agent workspace (defaults to cwd-inferred, then default agent)")
    .option("--as <slug>", "Install a git/local skill under this slug")
    .action(
      async (
        slug: string,
        opts: {
          version?: string;
          force?: boolean;
          global?: boolean;
          agent?: string;
          as?: string;
        },
        command: Command,
      ) => {
        try {
          const workspaceDir = resolveClawHubTargetWorkspaceDir(command, opts);
          if (!workspaceDir) {
            return;
          }
          if (isSkillSourceInstallSpec(slug)) {
            if (opts.version) {
              defaultRuntime.error("--version is only supported for ClawHub skill installs.");
              defaultRuntime.exit(1);
              return;
            }
            const result = await installSkillFromSource({
              workspaceDir,
              spec: slug,
              slug: opts.as,
              force: Boolean(opts.force),
              logger: {
                info: (message) => defaultRuntime.log(message),
                warn: (message) => defaultRuntime.log(theme.warn(message)),
              },
            });
            if (!result.ok) {
              defaultRuntime.error(result.error);
              defaultRuntime.exit(1);
              return;
            }
            defaultRuntime.log(
              `Installed ${result.slug} from ${result.source} -> ${result.targetDir}`,
            );
            return;
          }
          if (opts.as) {
            defaultRuntime.error(
              "--as is only supported for git and local directory skill installs.",
            );
            defaultRuntime.exit(1);
            return;
          }
          const result = await installSkillFromClawHub({
            workspaceDir,
            slug,
            version: opts.version,
            force: Boolean(opts.force),
            logger: {
              info: (message) => defaultRuntime.log(message),
            },
          });
          if (!result.ok) {
            defaultRuntime.error(result.error);
            defaultRuntime.exit(1);
            return;
          }
          defaultRuntime.log(`Installed ${result.slug}@${result.version} -> ${result.targetDir}`);
        } catch (err) {
          defaultRuntime.error(String(err));
          defaultRuntime.exit(1);
        }
      },
    );

  skills
    .command("update")
    .description("Update ClawHub-installed skills in the active or shared managed directory")
    .argument("[slug]", "Single skill slug")
    .option("--all", "Update all tracked ClawHub skills", false)
    .option("--global", "Update skills in the shared managed skills directory", false)
    .option("--agent <id>", "Target agent workspace (defaults to cwd-inferred, then default agent)")
    .action(
      async (
        slug: string | undefined,
        opts: { all?: boolean; global?: boolean; agent?: string },
        command: Command,
      ) => {
        try {
          if (!slug && !opts.all) {
            defaultRuntime.error("Provide a skill slug or use --all.");
            defaultRuntime.exit(1);
            return;
          }
          if (slug && opts.all) {
            defaultRuntime.error("Use either a skill slug or --all.");
            defaultRuntime.exit(1);
            return;
          }
          const workspaceDir = resolveClawHubTargetWorkspaceDir(command, opts);
          if (!workspaceDir) {
            return;
          }
          const tracked = await readTrackedClawHubSkillSlugs(workspaceDir);
          if (opts.all && tracked.length === 0) {
            defaultRuntime.log("No tracked ClawHub skills to update.");
            return;
          }
          const results = await updateSkillsFromClawHub({
            workspaceDir,
            slug,
            logger: {
              info: (message) => defaultRuntime.log(message),
            },
          });
          for (const result of results) {
            if (!result.ok) {
              defaultRuntime.error(result.error);
              continue;
            }
            if (result.changed) {
              defaultRuntime.log(
                `Updated ${result.slug}: ${result.previousVersion ?? "unknown"} -> ${result.version}`,
              );
              continue;
            }
            defaultRuntime.log(`${result.slug} already at ${result.version}`);
          }
        } catch (err) {
          defaultRuntime.error(String(err));
          defaultRuntime.exit(1);
        }
      },
    );

  skills
    .command("verify")
    .description("Verify a ClawHub skill trust card")
    .argument("<slug>", "ClawHub skill slug")
    .option("--version <version>", "Verify a specific version")
    .option("--tag <tag>", "Verify a tag")
    .option("--json", "Output as JSON", false)
    .option("--global", "Use the shared managed skills directory for installed-version lookup")
    .option("--agent <id>", "Target agent workspace (defaults to cwd-inferred, then default agent)")
    .action(
      async (
        slug: string,
        opts: {
          version?: string;
          tag?: string;
          json?: boolean;
          global?: boolean;
          agent?: string;
        },
        command: Command,
      ) => {
        try {
          if (opts.version && opts.tag) {
            defaultRuntime.error("Use either --version or --tag.");
            defaultRuntime.exit(1);
            return;
          }
          const workspaceDir = resolveClawHubTargetWorkspaceDir(command, opts);
          if (!workspaceDir) {
            return;
          }
          const result = await verifySkillFromClawHub({
            workspaceDir,
            slug,
            version: opts.version,
            tag: opts.tag,
          });
          if (!result.ok) {
            defaultRuntime.error(result.error);
            defaultRuntime.exit(1);
            return;
          }
          if (opts.json) {
            defaultRuntime.writeJson(result);
            return;
          }
          const auditStatus = printSkillTrustVerification({
            slug: result.slug,
            resolvedFrom: result.resolvedFrom,
            registry: result.registry,
            trustCard: result.trustCard,
          });
          if (auditStatus !== "pass") {
            defaultRuntime.exit(1);
          }
        } catch (err) {
          defaultRuntime.error(String(err));
          defaultRuntime.exit(1);
        }
      },
    );

  skills
    .command("list")
    .description("List all available skills")
    .option("--json", "Output as JSON", false)
    .option("--eligible", "Show only eligible (ready to use) skills", false)
    .option("-v, --verbose", "Show more details including missing requirements", false)
    .option("--agent <id>", "Target agent workspace (defaults to cwd-inferred, then default agent)")
    .action(
      async (
        opts: { json?: boolean; eligible?: boolean; verbose?: boolean; agent?: string },
        command: Command,
      ) => {
        await runSkillsAction((report) => formatSkillsList(report, opts), {
          agentId: resolveAgentOption(command, opts),
        });
      },
    );

  skills
    .command("info")
    .description("Show detailed information about a skill")
    .argument("<name>", "Skill name")
    .option("--json", "Output as JSON", false)
    .option("--agent <id>", "Target agent workspace (defaults to cwd-inferred, then default agent)")
    .action(async (name: string, opts: { json?: boolean; agent?: string }, command: Command) => {
      await runSkillsAction((report) => formatSkillInfo(report, name, opts), {
        agentId: resolveAgentOption(command, opts),
      });
    });

  skills
    .command("check")
    .description("Check which skills are ready, visible, or missing requirements")
    .option("--agent <id>", "Target agent workspace (defaults to cwd-inferred, then default agent)")
    .option("--json", "Output as JSON", false)
    .action(async (opts: { json?: boolean; agent?: string }, command: Command) => {
      await runSkillsAction((report) => formatSkillsCheck(report, opts), {
        agentId: resolveAgentOption(command, opts),
      });
    });

  // Default action (no subcommand) - show list
  skills.action(async (opts: { agent?: string }, command: Command) => {
    await runSkillsAction((report) => formatSkillsList(report, {}), {
      agentId: resolveAgentOption(command, opts),
    });
  });
}
