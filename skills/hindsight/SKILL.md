---
name: hindsight
description: Use the self-hosted Hindsight memory system for bank setup, retain/recall/reflect workflows, ingestion planning, query tuning, and operator checks. Trigger when working with Hindsight banks, memory ingestion, memory retrieval, the `hindsight` CLI, or the hosted deployment at `https://hindsight.alain.codes`.
homepage: https://hindsight.vectorize.io/
metadata: { "openclaw": { "emoji": "🧠", "requires": { "bins": ["hindsight", "hindsight-api"] } } }
---

# Hindsight

Use this skill when the task is about Hindsight usage, not generic vector search theory.

## References

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
2. Start with the relevant canonical doc from `references/global-docs.md`.
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
