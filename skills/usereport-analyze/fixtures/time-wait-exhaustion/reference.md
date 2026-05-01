# Analysis: time-wait-exhaustion

## TL;DR
TIME_WAIT socket exhaustion is causing 847 connect failures per interval; the ephemeral port range is likely depleted.

## Ranked Root Cause
TIME_WAIT exhaustion from short-lived outbound connection churn. [Finding: time_wait_exhaustion]

- TIME_WAIT count: 31,200 (threshold >28,000) [Signal: net.tw_count]
- Connect failures: 847 [Signal: net.connect_failures]
- TCP retransmit rate: 2.4% (secondary symptom) [Signal: net.retrans_pct]

## Alternative Hypotheses
1. **Ephemeral port range too narrow** — default `ip_local_port_range` may be 32,768–60,999 (only ~28k ports).
   Disambiguate: `sysctl net.ipv4.ip_local_port_range` [Finding: time_wait_exhaustion].
2. **High connection rate to a single backend** — TIME_WAIT count per destination, not global, causing failures.
   Disambiguate: `ss -tan state time-wait | awk '{print $5}' | sort | uniq -c | sort -rn | head`.

## Ordered Next-Step Commands
1. `sysctl net.ipv4.tcp_tw_reuse` [Finding: time_wait_exhaustion]
2. `sysctl net.ipv4.ip_local_port_range` [Finding: time_wait_exhaustion]
3. `ss -s` [Finding: time_wait_exhaustion]
4. `ss -tin` [Finding: net.high_retrans]
5. `netstat -s | grep retransmit` [Finding: net.high_retrans]

## Ruled Out
- Memory pressure: free_pct=55% [Signal: mem.free_pct]
- CPU saturation: usr_pct=22%, load=2.1 [Signal: cpu.usr_pct]
