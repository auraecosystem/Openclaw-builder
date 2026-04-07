# Rebase Plan

## Answer

The replay rebase is complete.

As of **2026-04-07**, the active replay branch is:

- branch: `codex/replay-main-20260407`
- head: `a5f2cc137a695e85bc47ed312d5b38bfd6a12e80`
- relation to upstream: `origin/main...HEAD = 0 behind / 81 ahead`

The original local `main` was preserved untouched. The replay was performed on a
fresh branch from latest `origin/main`, with safety refs left in place.

This file is no longer a speculative plan. It is now the working record of:

1. what the original plan was trying to achieve
2. what actually happened during the replay
3. why each important local change exists
4. where we intentionally kept local behavior versus accepting upstream
5. what validation debt and cleanup still remain

## Current Snapshot

- repo: `openclaw`
- replay branch: `codex/replay-main-20260407`
- replay head: `a5f2cc137a` `fix: keep memory flush replies internal`
- upstream target: `origin/main`
- replay branch status vs upstream:
  - ahead: `81`
  - behind: `0`
- safety refs:
  - tag: `rebase-safety-main-20260407`
  - branch: `codex/pre-replay-main-20260407`
- current worktree state when this file was updated:
  - modified: `pnpm-lock.yaml`
  - untracked: `REBASE-PLAN.md`

## What The Plan Required

The original plan was written after discovering that local `main` was:

- `ahead 81`
- `behind 6394`

The plan correctly concluded that the safe path was:

1. do **not** rebase local `main` directly
2. keep local `main` as the safety copy
3. create a fresh replay branch from latest `origin/main`
4. replay local work intentionally
5. prefer upstream if it already fixed the same problem more cleanly

That strategy was correct, and it is the part that mattered most.

## What Actually Happened

### What Matched The Plan

- We did **not** rewrite local `main`.
- We created safety refs before replaying.
- We replayed on a clean branch from latest `origin/main`.
- We used `rerere` and manual conflict resolution where appropriate.
- We paid the most attention to the production runtime fixes:
  - Discord mention handling
  - ACP finalization behavior
  - attachment-send correctness
  - stale sandbox session forking
  - model picker / streaming behavior
  - memory flush isolation

### What Deviated From The Plan

The plan recommended a selective replay:

- replay only the local changes still needed
- split runtime fixes from skills/docs
- strongly consider leaving algotrader / engine / research work on a separate
  branch

That did **not** happen. In practice, we replayed all 81 local commits.

So the end result is:

- a successful full replay of the old local branch onto latest upstream
- **not** a minimal audited branch containing only the fixes that upstream did
  not already subsume

That distinction matters for future cleanup.

## Safety Outcome

The replay was done safely.

Safety refs that must remain available until the branch is fully validated:

- `rebase-safety-main-20260407`
- `codex/pre-replay-main-20260407`

Operational meaning:

- the original local `main` is still recoverable
- the replay branch can still be compared against the preserved pre-replay
  branch

## Final Replay Branch History

Ordered replayed commits currently on top of `origin/main`:

1. `94bee39e7b` `feat(memory): add agent-framework memory plugin`
2. `11d7f35515` `fix(memory): make apiKey optional, use CLI subprocess, OrbStack URL`
3. `4b32f48320` `feat(extensions): add memory-agent-framework plugin`
4. `cf3a7c2834` `feat(skills): add finance skill with dashboard and references`
5. `a2b6049b7b` `feat(algotrader): add Python libraries and backtesting scripts`
6. `ab27f533a8` `feat(algotrader): add trading reference materials`
7. `b5d967bb31` `feat(algotrader): add NautilusTrader integration`
8. `a9ed3b05f2` `feat(algotrader): add Rust backtesting engine`
9. `86d99813f7` `feat(algotrader): add skill docs, methodology, and tests`
10. `ddc2d65567` `docs: update AGENTS.md`
11. `4eeb1229f9` `feat(engine): add engine-types crate with config structs`
12. `a047fe3bb3` `feat(engine): add engine-data crate isolating polars`
13. `2defe10b7c` `feat(engine): add engine-signals crate with flattened algorithms`
14. `0719e62962` `refactor(engine): wire workspace, update imports, remove old modules`
15. `0986c7de21` `docs(algotrader): update all docs for workspace refactor`
16. `44c45061f0` `feat(engine): add fuzzy pattern scoring with NEON acceleration`
17. `1fb43b32c6` `feat(engine): wire PatternParams into evolution + add config file loading`
18. `cdbdffda34` `feat(engine): data-agnostic engine, runtime indicators, multi-strategy portfolio`
19. `85bdcb8ae2` `feat(algotrader): venue-agnostic strategy, externalize all config params`
20. `6e02b5c6b8` `feat(ibkr): add REST daemon and bar fetching scripts`
21. `e7a8669170` `feat(nautilus): add IBKR backtest and live trading runners`
22. `3c7a1f0b63` `docs(algotrader): update CLAUDE.md with IBKR integration, refresh references`
23. `b1d0fc2972` `data(ibkr): add IBKR historical bar datasets`
24. `64696060f3` `docs: add cross-disciplinary precursor detection reference`
25. `7997dee731` `feat(engine): add config-driven strategy pipeline system`
26. `37b2e0f615` `docs(algotrader): update CLAUDE.md for dynamic pipeline system`
27. `5a8f8bc2b1` `feat(engine): 1:1 Qullamaggie strategy replication in dynamic pipeline`
28. `919d55c556` `feat(engine): add --compare flag with param override injection for fair parity testing`
29. `e747b6a003` `feat(engine): compute RS pctrank on-the-fly, fix breakout NaN handling`
30. `3ff40b9135` `chore(engine): add strum dev-dep to lockfile, optimize dev profile`
31. `962fb62871` `docs(algotrader): add EXP-011/012/013, update experiment table`
32. `961e874644` `feat(engine): JIT compilation, column elimination, and filter fusion`
33. `17f704a557` `docs(algotrader): EXP-012/014 narratives, optimize NT indicators`
34. `5eb1b8f88a` `chore: tidy .gitignore`
35. `679fbfb7ff` `Add stock trading toolbox skills`
36. `327c63d8f2` `Improve Discord reply handling and trading skills`
37. `eeec01a1c1` `Update local agent guidance and login tooling`
38. `a13e6b65d5` `Snapshot workspace before trading-data consolidation`
39. `bc6d005dee` `Update trading tool skills`
40. `4eec00b97b` `Refactor algotrader for config-driven trading paths`
41. `96ffe0eba4` `Add hash-permutations skill`
42. `cbc30ac483` `skills: point assistant-bot docs to trading-tools app path`
43. `5027c4e664` `docs(skills): refocus algotrader and clarify skill paths`
44. `c8775ebfbb` `chore(skills): archive algotrader as docs only`
45. `59f2744a40` `docs(skills): update trading-tools skill references`
46. `f7be509dd6` `skills: update assistant-bot trading workflows`
47. `ac80f62bdb` `discord: quiet no-reply mentions and dedupe skill commands`
48. `3705bdf27c` `docs: update trading integration skills`
49. `b0ee32335b` `refactor: rename skill paths to underscores`
50. `f24f37fcd1` `docs(skills): refresh tws scanner guidance`
51. `f6098d724b` `feat(extensions): add assistant-bot gateway sidecar`
52. `86605c1732` `feat(extensions): supervise trade daemon and assistant bot`
53. `aecd24b877` `docs(skills): expand stock charting annotations`
54. `b0564e542f` `fix(extensions): load trade db settings from env files`
55. `aa5a8c4ca5` `Update trading workflow skills`
56. `f870f489aa` `Sync trading skills with watch runtime`
57. `72e00e689f` `Sync skills with renamed trading CLIs`
58. `21b257e77e` `Relink Qullamaggie docs to knowledge base`
59. `b4b92fa30a` `add engineering_traceability skill`
60. `790b927c00` `fix: restore discord mentions and acp finalization`
61. `ea7ad3c8df` `docs: note rerere rebase conflicts`
62. `6f1935462a` `fix: harden attachment sends for crabman`
63. `4d980cfa4d` `Document specialist routing and ignore Rust targets`
64. `1177fde130` `Add Hindsight skill`
65. `201b405aae` `Link Hindsight skill to retained item template`
66. `5063f014ad` `Update Hindsight skill for direct list checks`
67. `da11200869` `fix: keep discord subagents on safe runs`
68. `2bc7bc50b7` `fix: avoid stale sandbox thread forks`
69. `f52fb91c77` `Remove algotrader skill and update trading references`
70. `8e9173038a` `Refine stock chart review format guidance`
71. `637b6e7db3` `Refine Hindsight skill guidance`
72. `6b1a75b1ad` `Route agents to Hindsight when appropriate`
73. `a9e512b96e` `docs(tws-skill): document IBKR crypto contract rules`
74. `f1defd7cb0` `docs(skills): align trading skills with canonical watch model`
75. `a493eb9d13` `docs(tws-skill): update live default and ETH quote guidance`
76. `db161fcf81` `docs(skills): align trading skills with post-Wave-4 runtime (#57462)`
77. `f59d3417e2` `docs(skills): align trading skills with hardened watch model`
78. `d294f58124` `Tighten prompt guidance for concise replies`
79. `4ee7f1ad29` `Update stock trading Qullamaggie references`
80. `5750f3f562` `fix: improve discord picker diagnostics and streaming`
81. `a5f2cc137a` `fix: keep memory flush replies internal`

## Critical Runtime Fixes We Carried Forward

This is the most important context for future conflict resolution.

### 1. Discord Mention Handling And ACP Finalization

Replay commits:

- `327c63d8f2`
- `ac80f62bdb`
- `790b927c00`

Why we changed this area locally:

- direct Discord mentions were crashing or silently disappearing
- ACP block output could be followed by duplicate final text
- explicit Discord mentions needed to remain quiet for benign no-reply cases

How it was resolved during replay:

- we did **not** blindly take the old files
- we merged into the newer upstream structure
- we preserved:
  - direct-mention silent-reply guarding
  - ACP block-output/final-output behavior
  - skill-command dedupe behavior

Important note:

- upstream had already changed the same subsystem heavily
- test expectations in this area were already drifting relative to production
  behavior
- this area still needs targeted post-replay validation

### 2. Attachment Sends Must Not Fake Success

Replay commit:

- `6f1935462a`

Why we changed this area locally:

- Crabman could call `message action=send` with `buffer/filename/contentType`
- OpenClaw would send text only
- the bot would claim it had attached the file when it had not

How it was resolved during replay:

- we kept the hardening behavior and tool/prompt guidance
- we adapted the test merge to current upstream prompt/test structure

What matters semantically:

- invalid upload shapes must not silently succeed
- `sendAttachment` should be the explicit upload path

### 3. Safe Subagents In Discord

Replay commit:

- `da11200869`

Why we changed it locally:

- thread-bound specialist handoff in Discord was failing live
- we added a tactical prompt-level workaround:
  plain subagent runs, no `thread:true`, manual relay

How it was resolved during replay:

- the local workaround was carried forward as prompt guidance

Important note:

- this is still a workaround, not a principled long-term architecture fix
- upstream had moved in this area, so this should be re-evaluated after runtime
  validation

### 4. Stale Sandbox Thread Forks

Replay commit:

- `2bc7bc50b7`

Why we changed it locally:

- thread continuations inherited stale sandbox-era cwd/context
- that broke tools such as TWS by keeping them pointed at dead sandbox paths

How it was resolved during replay:

- we ported the stale-workspace guard into the current session forking code
- the focused thread-fork regression passed during replay

Assessment:

- this remains one of the strongest production fixes we carried forward

### 5. Model Picker Diagnostics And Discord Streaming

Replay commit:

- `5750f3f562`

Why we changed it locally:

- Discord `/models` failures were opaque
- commentary/progress messages were bunching up and dumping late

How it was resolved during replay:

- runtime logging and commentary routing behavior were carried into the newer
  upstream structure
- the newly added model-picker persistence test from the old branch was dropped
  because it did not fit the current upstream picker internals cleanly

Assessment:

- the runtime behavior change was preserved
- the picker test coverage is still weaker than ideal

### 6. Memory Flush Must Not Steal User Replies

Replay commit:

- `a5f2cc137a`

Why we changed it locally:

- pre-compaction memory flush used the same reply lane as the real user turn
- maintenance `NO_REPLY` could consume the Discord reply slot

How it was resolved during replay:

- we preserved the runtime fix:
  - maintenance flush gets isolated reply state
  - outward reply callbacks are disabled
  - message tool is disabled for the flush run
- the e2e test file was merged into the latest upstream shape

Important note:

- current upstream memory-flush tests already assume a different execution shape
- the old local two-run e2e expectation is now stale
- the runtime fix is present, but the test coverage needs fresh adaptation

## Generated And Derived File Handling

### `pnpm-lock.yaml`

Current status:

- `pnpm install` succeeds on the replay branch
- it rewrites `pnpm-lock.yaml`
- the observed delta is a new importer entry:
  - `extensions/assistant-bot-sidecar: {}`

Interpretation:

- installability is good
- the branch is not install-clean until we decide whether to keep or revert that
  lockfile normalization

### `src/plugins/bundled-plugin-metadata.generated.ts`

Replay handling:

- bundled metadata churn from local plugin additions was resolved during replay
- generated plugin metadata should still be treated as a regenerate-first file in
  any future replay

## Validation Outcome So Far

### What We Confirmed During Replay

- the replay itself completed cleanly onto latest upstream
- thread-fork regression passed after the stale sandbox fix was merged
- attachment-related targeted tests passed at the time that fix was merged
- prompt-composition / system-prompt targeted tests passed after their merge
- Discord/model-picker runtime merge completed without leaving unresolved
  conflicts
- `pnpm install` works on the replay branch

### What Is Still Not Fully Green

The branch is replayed, but not fully validated to the standard of the original
plan.

Known remaining validation debt:

- memory-flush e2e expectations are stale against latest upstream behavior
  - even an older upstream memory-flush test now sees only one embedded run, not
    the previously expected two-run shape
- ACP-related expectations were also stale relative to the preserved production
  behavior during replay
- the model-picker regression coverage is still not as strong as the original
  plan wanted
- full `pnpm check` and full `pnpm test` have **not** been completed on the
  replay branch

## What Went According To Plan

- safe replay branch strategy
- preservation of original `main`
- preservation of safety refs
- completion onto latest `origin/main`
- manual conflict handling in high-risk subsystems
- bias toward protecting known production fixes

## What Did Not Go According To Plan

- we replayed all 81 commits rather than selectively carrying only what was
  still needed
- we did not split the branch into runtime wave versus local-content wave
- we did not fully re-audit every overlapping subsystem to determine whether
  upstream had already subsumed the local fix
- we did not finish the post-replay validation pass before declaring the replay
  complete

## Remaining Work

### Immediate Cleanup

- [ ] decide whether to keep the `pnpm-lock.yaml` normalization
- [ ] keep or commit this updated `REBASE-PLAN.md`

### Post-Replay Validation

- [ ] rework memory-flush e2e expectations to current upstream execution shape
- [ ] re-run targeted ACP / mention validation
- [ ] strengthen model-picker persistence coverage if still needed
- [ ] run `pnpm check`
- [ ] run `pnpm test`

### Optional Simplification Work

If the goal becomes “minimize divergence from upstream,” do this next:

- [ ] audit whether the prompt-level Discord subagent workaround is still needed
- [ ] audit whether the replayed picker / streaming delta can be reduced further
- [ ] consider moving algotrader / engine / research history back out to a
      separate branch if the runtime branch should stay lean

## Future Conflict Rules

These rules remain valid for the next upstream rebase.

### Rule 1: Prefer Upstream When It Clearly Solves The Same Incident

Take upstream if all are true:

- same user-visible bug
- simpler implementation
- equivalent or stronger coverage
- no regression in the local observed behavior we cared about

### Rule 2: Keep Local Behavior When The Incident Was Real And Upstream Still Does Not Cover It

This especially applies to:

- stale sandbox thread forks
- attachment-send honesty
- memory-flush isolation

### Rule 3: Treat Prompt Workarounds As Tactical, Not Sacred

Prompt-level operational rules should be easier to drop than runtime fixes.

### Rule 4: Generated Files Should Be Rebuilt, Not Hand-Merged

Still applies to:

- `pnpm-lock.yaml`
- generated plugin metadata

## Session Handoff Summary

If a future session needs the shortest useful context:

- the full replay onto latest `origin/main` is done
- replay branch: `codex/replay-main-20260407`
- original local `main` is preserved
- safety refs still exist
- critical local runtime fixes were carried forward
- this was a **full replay**, not a minimal audited replay
- the branch still has validation debt:
  - stale memory-flush tests
  - incomplete full-suite validation
  - unresolved decision on `pnpm-lock.yaml`
