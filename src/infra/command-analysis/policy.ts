import { splitShellArgs } from "../../utils/shell-argv.js";
import {
  analyzeArgvCommand,
  type ExecCommandAnalysis,
  type ExecCommandSegment,
} from "../exec-approvals-analysis.js";
import { detectInlineEvalInSegments } from "./risks.js";

export type CommandPolicyAnalysis =
  | {
      ok: true;
      source: "argv" | "shell";
      analysis: ExecCommandAnalysis;
      segments: ExecCommandSegment[];
    }
  | {
      ok: false;
      source: "argv" | "shell";
      reason?: string;
      analysis: ExecCommandAnalysis;
      segments: [];
    };

export function analyzeCommandForPolicy(
  params:
    | {
        source: "argv";
        argv: string[];
        cwd?: string;
        env?: NodeJS.ProcessEnv;
      }
    | {
        source: "shell";
        command: string;
        cwd?: string;
        env?: NodeJS.ProcessEnv;
      },
): CommandPolicyAnalysis {
  const analysis =
    params.source === "argv"
      ? analyzeArgvCommand({ argv: params.argv, cwd: params.cwd, env: params.env })
      : analyzeShellTextForPolicy({
          command: params.command,
          cwd: params.cwd,
          env: params.env,
        });
  if (!analysis.ok) {
    return {
      ok: false,
      source: params.source,
      reason: analysis.reason,
      analysis,
      segments: [],
    };
  }
  return {
    ok: true,
    source: params.source,
    analysis,
    segments: analysis.segments,
  };
}

export function detectPolicyInlineEval(segments: readonly ExecCommandSegment[]) {
  return detectInlineEvalInSegments(segments);
}

function analyzeShellTextForPolicy(params: {
  command: string;
  cwd?: string;
  env?: NodeJS.ProcessEnv;
}): ExecCommandAnalysis {
  const argv = splitShellArgs(params.command.trim());
  if (!argv || argv.length === 0) {
    return { ok: false, reason: "unable to parse shell command", segments: [] };
  }
  return analyzeArgvCommand({ argv, cwd: params.cwd, env: params.env });
}
