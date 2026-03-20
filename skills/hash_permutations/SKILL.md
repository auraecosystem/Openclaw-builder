---
name: hash-permutations
description: >
  Generate SHA-256 hashes for common human-typed variants of plaintext input
  lines using the workspace hash-permutations utility. Use when asked about:
  string mutation hashing, candidate generation for typed identifiers,
  password-style variant hashing, JSONL hash output, or jq-friendly
  normalization and hashing workflows.
metadata: { "openclaw": { "emoji": "🔐", "requires": { "bins": ["python3"] } } }
---

# hash-permutations

Use the workspace utility at `/Users/ad/.openclaw/workspace/scripts/hash_permutations.py`.

Primary files:

- `/Users/ad/.openclaw/workspace/scripts/hash_permutations.py`
- `/Users/ad/.openclaw/workspace/tests/test_hash_permutations.py`

## When to use

- "hash common variants of these strings"
- "generate SHA-256 hashes for likely typed forms"
- "turn these phrases into snake/camel/kebab/etc and hash them"
- "emit JSONL so I can pipe into jq"
- "build candidate hashes from a wordlist or plaintext file"

## Defaults

- Prefer JSONL output for large runs and shell pipelines.
- Per-line dedupe is on by default.
- Blank input lines are skipped by default.
- Empty transformed values are skipped by default.
- Use `--summary` when the user wants counts or job metadata.

## Quick start

Basic run:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt
```

JSONL with summary:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt --summary
```

Write JSON document output:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt --format json --output out.json
```

Restrict transforms:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt --transforms original,snake_case,camel_case
```

## jq patterns

All hashes:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt | jq -r 'select(.type=="candidate") | .sha256'
```

Only snake case:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt | jq 'select(.type=="candidate" and .transform=="snake_case")'
```

Unique candidate values:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt | jq -r 'select(.type=="candidate") | .value' | sort -u
```

Group candidates by source line:

```bash
python3 /Users/ad/.openclaw/workspace/scripts/hash_permutations.py input.txt | jq -s 'map(select(.type=="candidate")) | group_by(.line_number)'
```

## Safe workflow

1. Start with the default JSONL mode unless the caller explicitly needs one JSON document.
2. Keep per-line dedupe enabled unless duplicate transform rows are part of the analysis.
3. Use `--transforms` to narrow the output when the caller only cares about a few naming styles.
4. Use `--output` for larger runs instead of copying large stdout payloads back into chat.

## Known constraints

- The tool hashes common practical typing variants, not every possible mutation.
- Unicode transliteration is not implemented in v1.
- Deduplication is per source line, not global across the full file.
- Parallelism is by line, so gains are best on multi-line inputs.

## Output handling

- Default output is JSONL candidate rows.
- Summary rows are only appended when `--summary` is requested.
- `--format json` emits a single object with `summary` and `candidates`.
