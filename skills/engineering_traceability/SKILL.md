---
name: engineering-traceability
description: Enforce end-to-end engineering traceability across GitHub for all coding work. Use when planning or executing code changes, reporting bugs, creating TODO/roadmap items, writing commits, opening PRs, triaging reviews, managing labels/milestones/projects, or auditing delivery history. Also use when capturing pre-roadmap ideas via GitHub Discussions and promoting them into Issues when actionable.
---

# Engineering Traceability

## Overview
Ensure every change is traceable from idea -> issue -> branch -> commit -> PR -> review -> merge -> follow-up.

## Workflow

### 1) Classify incoming work item (before coding)
- **Discussion**: exploratory idea, uncertainty, no immediate build step.
- **Issue**: actionable TODO/bug/feature.
- **PR**: implementation unit tied to one or more issues.

Rule: no meaningful coding without an Issue.

### 2) Discussion-first flow for ideas
Use GitHub Discussions for non-actionable ideas:
1. Create Discussion (Ideas/Architecture/Research).
2. Capture problem, constraints, success criteria, unknowns.
3. Tag with area/risk/priority in body.
4. Promote to Issue(s) once actionable.
5. Back-link Issue(s) to Discussion.

Template: `references/discussion-template.md`.

### 3) Issue standards (TODO/roadmap/bug)
Every actionable item must be an Issue with:
- problem statement
- scope boundaries (in/out)
- acceptance criteria
- test evidence expected
- rollback considerations
- priority + labels + milestone

Templates:
- `references/issue-template.md`
- `references/bug-report-template.md`

### 4) Commit guidelines
Use small, reviewable commits:
- target 1 logical change per commit
- avoid mixed concerns (refactor + feature + formatting in one commit)
- reference Issue ID in commit subject/body

Preferred style:
- `<type>(<scope>): <summary> (#<issue>)`
- types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

Examples:
- `fix(risk): enforce max position cap (#142)`
- `refactor(news): extract source budget policy (#155)`

Commit size guidance:
- Aim: <= 300 changed LOC per commit when practical
- If larger, split by concern and preserve build/test integrity

Reference: `references/commit-guidelines.md`.

### 5) PR scope/size guidelines
PRs should map to one coherent objective.

Required PR contents:
- linked issue (`Closes #...` when appropriate)
- summary + non-goals
- test evidence
- risk/safety impact
- rollback note
- links to related Discussion(s)

PR size guidance:
- Preferred: <= 600 changed LOC
- If > 600 LOC, include explicit decomposition rationale and review map
- Avoid “mega PRs” except urgent hotfixes

Reference: `references/pr-template.md`.

### 6) Review + merge traceability
- Resolve review comments with commit references.
- Preserve review history; avoid destructive rebases unless required.
- Ensure CI status is visible in PR.
- Keep merge message issue-linked.

### 7) Post-merge closure
For each merged PR, verify full chain:
Discussion (if any) -> Issue -> Branch -> Commit(s) -> PR -> Merge SHA -> Follow-up Issue(s)

Checklist: `references/traceability-checklist.md`.

## TODO / Roadmap logging rules
- TODOs in chat/docs must be mirrored to GitHub (Discussion or Issue).
- Use Issues for executable work now.
- Use Discussions for exploratory work, then promote.
- Roadmap epics should be milestone-backed and decomposed into Issues.

Reference: `references/todo-roadmap-guidelines.md`.

## GitHub feature mapping (required)
- Discussions: incubation
- Issues: executable work
- Projects: workflow board
- Labels: area/type/priority/risk
- Milestones: delivery windows
- PRs: implementation + review + CI evidence
- Commits: granular audit trail

## Guardrails
- No issue, no coding.
- No orphan TODOs in chat.
- No merge without linked issue and test evidence.
- No hidden scope creep in PRs; update issue if scope changes.

## Minimum audit report format
1. Open Discussions (unpromoted)
2. Open Issues by priority
3. In-progress PRs + CI state
4. Recently merged PRs + closed issues
5. Broken trace links

## References
- `references/discussion-template.md`
- `references/issue-template.md`
- `references/bug-report-template.md`
- `references/commit-guidelines.md`
- `references/pr-template.md`
- `references/todo-roadmap-guidelines.md`
- `references/traceability-checklist.md`
