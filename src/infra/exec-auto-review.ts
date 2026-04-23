import { normalizeLowercaseStringOrEmpty } from "../shared/string-coerce.js";

export type ExecAutoReviewRisk = "unknown" | "low" | "medium" | "high";

export type ExecAutoReviewDecision =
  | {
      decision: "allow-once";
      rationale: string;
      risk: "low" | "medium" | "high";
    }
  | {
      decision: "deny";
      rationale: string;
      risk: "medium" | "high";
    }
  | {
      decision: "ask-human";
      rationale: string;
      risk: ExecAutoReviewRisk;
    };

export type ExecAutoReviewHost = "gateway" | "node";

export type ExecAutoReviewInput = {
  command: string;
  argv?: readonly string[];
  cwd?: string | null;
  envKeys?: readonly string[];
  host: ExecAutoReviewHost;
  reason: "approval-required" | "allowlist-miss" | "strict-inline-eval" | "execution-plan-miss";
  analysis: {
    parsed: boolean;
    allowlistMatched: boolean;
    safeBinMatched?: boolean;
    durableApprovalMatched?: boolean;
    inlineEval: boolean;
    shellWrapper?: boolean;
  };
  agent?: {
    id?: string | null;
    sessionKey?: string | null;
  };
};

export type ExecAutoReviewer = (
  input: ExecAutoReviewInput,
) => Promise<ExecAutoReviewDecision> | ExecAutoReviewDecision;

const READ_ONLY_ZERO_ARG_BINARIES = new Set(["pwd"]);

function commandLooksCompound(command: string): boolean {
  return /(^|[^\\])(?:&&|\|\||;|\||>|<|`|\$\()|\r|\n/u.test(command);
}

function normalizeBinary(argv: readonly string[] | undefined, command: string): string {
  const first = argv?.[0] ?? command.trim().split(/\s+/u)[0] ?? "";
  return normalizeLowercaseStringOrEmpty(first);
}

function hasDangerousToken(command: string): boolean {
  return /\b(?:chmod\s+777|chown|curl|dd|mkfs|mv|npm\s+publish|pnpm\s+publish|rm|rsync|scp|ssh|sudo|wget)\b/iu.test(
    command,
  );
}

function isReadOnlyCommand(input: ExecAutoReviewInput): boolean {
  const binary = normalizeBinary(input.argv, input.command);
  return READ_ONLY_ZERO_ARG_BINARIES.has(binary) && (input.argv?.length ?? 0) === 1;
}

export const defaultExecAutoReviewer: ExecAutoReviewer = (input) => {
  if (!input.analysis.parsed || input.analysis.inlineEval || input.analysis.shellWrapper) {
    return {
      decision: "ask-human",
      rationale: "command shape needs explicit operator review",
      risk: "medium",
    };
  }
  if (commandLooksCompound(input.command) || hasDangerousToken(input.command)) {
    return {
      decision: "ask-human",
      rationale:
        "command includes mutation, network, shell composition, or privilege-sensitive tokens",
      risk: "high",
    };
  }
  if (isReadOnlyCommand(input)) {
    return {
      decision: "allow-once",
      rationale: "single zero-argument inspection command",
      risk: "low",
    };
  }
  return {
    decision: "ask-human",
    rationale: "no native auto-review rule matched",
    risk: "unknown",
  };
};
