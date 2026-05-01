# usereport-analyze

Analyze a `usereport --output llm` document and produce a structured performance diagnosis.

## Input Contract

Accepts a `usereport --output llm` JSON document from:
- **stdin**: `usereport --output llm | claude /usereport-analyze`
- **file argument**: `claude /usereport-analyze report.json`

## Schema Version Check

**REQUIRED**: Before any analysis, read the `schema_version` field.

- If `schema_version` is `"1"`: proceed with analysis.
- If `schema_version` is anything else: **refuse analysis** and output exactly:
  ```
  Error: unsupported schema_version "<value>". This skill requires schema_version "1".
  ```
  Do not attempt to interpret the document.

## Output Structure

Produce the following sections in order:

### 1. TL;DR (one sentence)
Summarize the most critical finding in one sentence.

### 2. Ranked Root Cause
State the most likely root cause. Cite the `id` of the specific finding or signal from the input document. Example: `[Finding: dmesg.oom_kill]` or `[Signal: mem.free_pct = 2.3%]`.

### 3. Alternative Hypotheses
List 1–3 alternative explanations ranked by likelihood. For each, state what evidence would disambiguate it from the primary hypothesis.

### 4. Ordered Next-Step Commands
List the `suggest` commands from the relevant findings, in priority order. Do not invent commands not present in the document.

### 5. Ruled Out
List signals and findings that were investigated and do not explain the observed behaviour. Cite signal IDs or finding IDs from the input.

## Hard Constraints

**Never fabricate metric values.** Every numeric claim must cite a `finding.id`, `signal.id`, or `finding.evidence[].signal_id` from the input document. If a metric is not in the document, do not state it.

**Never invent findings.** Only reference `id` values that appear literally in the `findings` array of the input document.

**Never speculate beyond the data.** If the document lacks enough signals to disambiguate between hypotheses, say so explicitly rather than guessing.

## Fixtures

Reference examples are in `fixtures/` alongside this file. Each directory contains:
- `input.json`: a schema-valid `usereport --output llm` document
- `reference.md`: the expected analysis output (ground truth for CI evaluation)

Fixtures: `good-box/`, `memory-pressure/`, `io-bound/`, `thermal-throttle/`, `time-wait-exhaustion/`.
