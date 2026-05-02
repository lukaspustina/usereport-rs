# SDD Implementation Report: more-useful-firefight

**Date**: 2026-05-02
**Phases run**: 1, 2, 3, 4, 5
**Overall status**: all-shipped

| Phase | Title | Status | Commit |
|-------|-------|--------|--------|
| 1 | Install Hints & Command Metadata | shipped | 0c27f3d |
| 2 | Coverage Gaps, Healthy Section, Threshold Column | shipped | ff2e5fe |
| 3 | Diagnostic Thread — Findings to Source Commands | shipped | 408b37b |
| 4 | At-a-Glance Overview | shipped | e402207 |
| 5 | Output-Derived Signals | shipped | 6dbe45c |

## Manual Test Plan

1. **Coverage Gaps** — run `usereport` on a system missing `sysstat`; verify the markdown output contains a "Coverage Gaps" table listing the missing binary and install hint.
   Expected: table row with `sar_cpu | sar | apt-get install sysstat`.

2. **Healthy section** — run `usereport` on a healthy system; verify a "Healthy" section appears listing signal IDs that were checked but didn't fire.
   Expected: at least one signal ID (e.g. `cpu.iowait_pct`) under "Healthy".

3. **Threshold column** — run `usereport --output json` and confirm `signal_thresholds` is present; render markdown and confirm the Signals table has a "Threshold" column.
   Expected: `| cpu.iowait_pct | ... | > 10.0 (Warn) |`

4. **Source-command links (HTML)** — render HTML report with `--output html`; open in browser and verify evidence items in findings have clickable `[command-name]` links that anchor to the command output section.
   Expected: `<a href="#cmd-sar_cpu">[sar_cpu]</a>` next to evidence.

5. **Vital Signs** — run `usereport` and verify the markdown output starts with a "Vital Signs" table showing CPU, Memory, Disk, Network rows.
   Expected: all four resource rows present; rows with no data show `[not profiled]`.

6. **USE Coverage** — run `usereport` on Linux with `sysstat` installed; verify "USE Coverage" table appears with ✓ for covered dimensions.
   Expected: `| network | ✓ | ✓ | ✗ |` (saturation covered by sar_tcp, errors covered by sar_edev, utilization by sar_dev).

7. **Follow-up recommendations** — create a report with `cpu.iowait_elevated` firing; verify "Where to investigate next" section appears with `mem` recommendation.
   Expected: `- **mem** — iowait often driven by memory pressure`.

8. **Diff tip** — verify the diff tip `usereport diff` appears only when findings are non-empty.
   Expected: present in a report with findings; absent in a healthy report.

9. **Extract signals** — run `usereport` on Linux; verify `dmesg.oom_count` appears in the Signals table when OOM events are present in dmesg.
   Expected: `dmesg.oom_count` with value in the signals table.

10. **Explain command** — run `usereport explain vmstat`; verify title, description, and what_to_look_for are shown.
    Expected: output contains "Virtual Memory statistics" and "iowait".

## How to Resume Blocked Phases

All phases shipped. No blocked phases.
