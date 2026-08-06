---
name: code-review
description: Use when the user asks to review code, find bugs, suggest improvements, or check code quality. Provides a systematic review checklist covering correctness, security, performance, and style.
metadata:
  allow-implicit-invocation: true
---

# Code Review

When the user asks for a code review, follow this systematic process:

## 1. Locate the code
- Ask which file(s) to review, or use `list_dir` / `read_file` to find them in the workspace.
- Read the full files before commenting. Never review from memory.

## 2. Review checklist
- **Correctness**: off-by-one errors, null/undefined handling, race conditions, edge cases, error paths.
- **Security**: injection risks, unsafe deserialization, hardcoded secrets, path traversal, missing auth checks.
- **Performance**: obvious algorithmic issues (O(n²) where O(n) is possible), blocking calls in hot paths, unbounded memory.
- **Maintainability**: dead code, magic numbers, unclear naming, missing abstractions, duplicated logic.
- **Style**: consistency with the surrounding codebase and language conventions.

## 3. Output format
For each issue found, report:
- Severity: **P0** (must fix) / **P1** (should fix) / **P2** (nice to have)
- Location: file + approximate line
- The problem, why it matters, and a concrete fix suggestion (with code snippet when helpful)

End with a summary: overall assessment and the top 3 priorities.

## Example structure

```markdown
## 审查结果

### P1 - 文件: src/auth.ts:42
**问题**: ...
**影响**: ...
**修复建议**: 
```ts
...
```
```
