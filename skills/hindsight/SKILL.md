---
name: hindsight
description: Use the self-hosted Hindsight memory system for bank setup, retain/recall/reflect workflows, ingestion planning, query tuning, and operator checks. Trigger when working with Hindsight banks, memory ingestion, memory retrieval, the `hindsight` CLI, or the hosted deployment at `https://hindsight.alain.codes`.
homepage: https://hindsight.vectorize.io/
metadata: { "openclaw": { "emoji": "🧠", "requires": { "bins": ["hindsight"] } } }
---

# Hindsight

Use this skill when the task is about Hindsight usage, not generic vector search theory.

## References

- `references/global-docs.md` for the canonical local docs map

## Workflow

1. Determine the task type:
   - operator check
   - bank design
   - ingestion planning
   - retain/recall query usage
   - deployment/configuration troubleshooting
2. Start with the relevant canonical doc from `references/global-docs.md`.
3. Prefer the official `hindsight` CLI for smoke tests and small manual actions.
4. Prefer the API or SDK for bulk ingestion, structured metadata generation, and manifest-driven backfills.
5. Apply the house ingestion rules from the global docs:
   - keep `metadata` lean
   - use stable `document_id`
   - use real timestamps or `"unset"` for timeless reference docs
   - use tags for scope
   - skip low-signal hubs, nav dumps, and duplicate canonicals
6. Use `recall` for grounding and `reflect` only when synthesis is actually needed.

## Local deployment facts

- Hosted URL: `https://hindsight.alain.codes`
- Local CLI binary: `/Users/ad/.local/bin/hindsight`
- Local CLI config: `/Users/ad/.hindsight/config`

## Guardrails

- Do not treat Hindsight as the primary exact code-search layer.
- Do not pre-summarize content before retain.
- Do not put ingestion diagnostics, hashes, or token counts into Hindsight `metadata`.
- Do not invent timestamps for timeless notes from file mtimes or ingest time.
- Do not assume a listed model or bank is usable without a real smoke test.
