---
name: doc-writer
description: Use when the user asks to write, improve, or translate documentation, README files, API docs, or technical articles. Produces clear, structured, audience-appropriate documentation.
metadata:
  allow-implicit-invocation: true
---

# Documentation Writer

Write documentation that people actually want to read.

## Process
1. **Clarify audience and goal**: who reads this, what do they need to do?
2. **Read the source material first**: for code docs, read the actual code/spec with `read_file`; never invent APIs.
3. **Draft structure** before writing long content.

## Structure guidance
- **README**: What → Why → Quick start → Usage → Config → Troubleshooting → License.
- **API docs**: one section per endpoint/function: signature, params table, return value, error cases, example.
- **Tutorials**: prerequisites → step-by-step numbered instructions → expected outcome per step → common pitfalls.

## Style rules
- Lead with the most important information (inverted pyramid).
- Use concrete examples over abstract descriptions.
- Prefer short sentences. Chinese docs: use 中文 for prose, keep code/API identifiers in English.
- Always include runnable code examples with realistic inputs/outputs.
- Tables for structured parameters; fenced code blocks with language tags.
- If documentation references files, verify the paths exist.

## Checklist before finishing
- [ ] Examples are real and tested
- [ ] No placeholder text like "TBD" or "TODO"
- [ ] Consistent terminology and formatting
- [ ] Has a clear title and table of contents for long docs
