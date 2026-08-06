---
name: data-analyzer
description: Use when the user asks to analyze data, compute statistics, read CSV/JSON data files, or interpret numbers and trends. Helps inspect datasets and produce accurate, evidence-based analysis.
metadata:
  allow-implicit-invocation: true
---

# Data Analysis

Analyze data files accurately and honestly.

## Process
1. **Inspect the data first**: use `list_dir` / `read_file` (or `bash` with `head`, `awk`, `wc` if enabled) to understand structure, size, columns, and sample rows. For CSVs, note the delimiter and header row.
2. **State assumptions**: encoding, missing values, date formats — make them explicit before computing.
3. **Compute carefully**: prefer exact counts and sums; round only when presenting. Show the formula or command used so results are reproducible.
4. **Report honestly**: distinguish facts from interpretation. If data is insufficient, say so. Never invent data points.

## Output format
- **Summary**: 2-3 sentence takeaway in plain language.
- **Key numbers**: a small table of the most relevant metrics.
- **Trends/insights**: what changed, what stands out, what is surprising.
- **Caveats**: data quality issues, sampling bias, missing dimensions.
- If applicable, suggest what additional data would strengthen the analysis.

## Rules
- Always quote the source file and, where possible, the row/line numbers behind each key number.
- For large datasets, analyze a representative sample and clearly label it as such.
- Use Markdown tables and fenced code for results; keep raw dumps minimal.
