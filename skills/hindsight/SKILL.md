---
name: hindsight
description: Use the self-hosted Hindsight memory system for bank setup, retain/recall/reflect workflows, ingestion planning, query tuning, and operator checks. Trigger when working with Hindsight banks, memory ingestion, memory retrieval, the `hindsight` CLI, or the hosted deployment at `https://hindsight.alain.codes`.
homepage: https://hindsight.vectorize.io/
metadata: { "openclaw": { "emoji": "🧠", "requires": { "bins": ["hindsight", "hindsight-api"] } } }
---

# Hindsight

Use this skill when the task is about Hindsight usage, not generic vector search theory.

## References

- `/Users/ad/dotfiles/docs/tools/hindsight/hindsight-usage-guide.md` for the primary local workflow guide
- `references/global-docs.md` for the canonical local docs map
- `/Users/ad/dotfiles/docs/tools/hindsight/hindsight-retained-item-template.md` for the manual retained-item template
- `/Users/ad/dotfiles/docs/tools/hindsight/hindsight-api-wrapper.md` for the JSON-first local retain wrapper

## Workflow

1. Determine the task type:
   - operator check
   - bank design
   - ingestion planning
   - retain/recall query usage
   - deployment/configuration troubleshooting
2. Start with `/Users/ad/dotfiles/docs/tools/hindsight/hindsight-usage-guide.md` unless the task is already clearly narrow and one of the specialized docs is the better entry point.
3. Prefer the official `hindsight` CLI for smoke tests, bank management, recall, reflect, and small operator actions.
4. Prefer `/Users/ad/bin/hindsight-api` for direct API gaps:
   - `retain` for manual atomic retains that need JSON input, tags, metadata, timestamps, or observation scope
   - `list` for reliable direct bank-content checks
5. Prefer the API or SDK for bulk ingestion, structured metadata generation, and manifest-driven backfills.
6. Apply the house ingestion rules from the global docs:
   - keep `metadata` lean
   - use stable `document_id`
   - use real timestamps or `"unset"` for timeless reference docs
   - use tags for scope
   - skip low-signal hubs, nav dumps, and duplicate canonicals
   - use the retained-item template when preparing a manual atomic ingest
   - dry-run the JSON request before sending it
   - verify retained content with `hindsight-api list <bank>`
7. Use `recall` for grounding and `reflect` only when synthesis is actually needed.
8. When a task is "how should this be ingested?", route through the retained-item template and the usage guide before writing any JSON.

## When to log to Hindsight

Do not log by reflex. Log only when the information is likely to save real work later.

Log it when all or almost all of these are true:

- it will still matter beyond the current turn or session
- it is verified, source-backed, or clearly framed as a current operator caveat
- it can be stated as one atomic fact, rule, workaround, or reference note
- it fits a specific bank mission
- a future agent could plausibly search for it directly
- it contains no secrets, credentials, or protected personal data

High-value logging candidates:

- durable CLI or API usage rules
- endpoint shape requirements and auth-key distinctions
- verified operator caveats, cleanup paths, timeout behavior, and deployment constraints
- durable tool-selection rules such as when to use `hindsight` versus `hindsight-api`
- stable workarounds for real bugs or platform quirks
- distilled lessons from live troubleshooting after the issue is understood

## When not to log to Hindsight

Do not log:

- turn-local planning or scratch thinking
- raw command transcripts, stack traces, or console spam without distillation
- unverified guesses, vague suspicions, or half-understood behavior
- duplicates of facts already in the bank
- giant mixed-topic summaries when one atomic note would do
- secrets, API keys, bearer tokens, cookies, private keys, or sensitive user data
- repo-local implementation details that belong in a `repo-*` bank or in source docs instead of a tool bank
- exact code-search material that should stay in grep, AST tools, or repo docs

If the item is not clearly durable, atomic, and retrieval-worthy, do not log it yet.

## Logging heuristics

Use this decision order:

1. Does this belong in memory at all, or only in the current conversation?
2. Is it durable enough to matter later?
3. Is it one coherent fact or rule?
4. Is it verified enough to trust?
5. Which bank actually owns it?
6. Can it be written cleanly without noise or secrets?

If any answer is weak, stop and either skip logging or tighten the retained item first.

Good logging shape:

- one retained item per lesson
- factual, tool-facing wording
- minimal metadata
- tags that match the real retrieval boundary

Bad logging shape:

- whole transcripts
- worklogs
- unresolved debugging dumps
- broad “here is everything we touched” summaries

## Local deployment facts

- Hosted URL: `https://hindsight.alain.codes`
- Local CLI binary: `/Users/ad/.local/bin/hindsight`
- Local JSON-first retain wrapper: `/Users/ad/bin/hindsight-api`
- Local CLI config: `/Users/ad/.hindsight/config`

## Guardrails

- Do not treat Hindsight as the primary exact code-search layer.
- Do not pre-summarize content before retain.
- Do not put ingestion diagnostics, hashes, or token counts into Hindsight `metadata`.
- Do not invent timestamps for timeless notes from file mtimes or ingest time.
- Do not assume a listed model or bank is usable without a real smoke test.
