# Commit Guidelines

## Format
`<type>(<scope>): <summary> (#<issue>)`

Types:
- feat, fix, refactor, docs, test, chore

## Rules
- One logical change per commit.
- Reference issue id in subject/body.
- Keep commits reviewable and reversible.
- Avoid mixing refactor + behavior changes unless explicitly noted.

## Size guidance
- Preferred <= 300 changed LOC per commit.
- If larger, split by concern while preserving testability.

## Quality gate before commit
- Tests relevant to changed files pass.
- Lint/format pass (project standard).
- Commit message explains *why*, not only *what*.
