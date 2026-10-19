# Benchmark results

Measured 2026-10-18 on an Apple M3 Max (16 cores), macOS, toolchain 1.98.1,
release profile. The machine was shared with other builds (load average
5–7), and the load generator, mock upstream and proxy ran on the same host,
so concurrent numbers include CPU contention. Re-run before quoting new
numbers.

## P1: account selection

`cargo bench -p relay-core --bench select -- --warm-up-time 1 --measurement-time 3`

| Case                               | Time (criterion median) |
| ---------------------------------- | ----------------------- |
| `select`, quota-aware, 16 accounts | 27.1 ns                 |

`select` takes a slice and allocates nothing; the request path builds one
small candidate `Vec` per decision (not measured separately).

## P2/P5: added latency, observed pass-through

`cargo bench -p relay-proxy --bench overhead` — same load direct to a local
SSE upstream and through Relay; streams are checked byte for byte.

| Load                      | Metric     | direct p50 µs | direct p99 µs | relay p50 µs | relay p99 µs | added p50 µs | added p99 µs |
| ------------------------- | ---------- | ------------- | ------------- | ------------ | ------------ | ------------ | ------------ |
| sequential, 256 B body    | first byte | 40            | 60            | 63           | 131          | +23          | +72          |
| 64 concurrent, 32 KB body | first byte | 484           | 1217          | 1027         | 2921         | +544         | +1704        |
| sequential, 2 MB body     | first byte | 284           | 560           | 974          | 1265         | +691         | +705         |

`telemetry_dropped = 0` across the run.

Against the proposed gates in `research/routing-architecture.md` §3: the
fixed per-request cost is ~23 µs p50. At 64 concurrent requests on this
loaded host, added p50 was 0.54 ms (gate 0.5 ms, **missed** by 0.04 ms) and
added p99 1.7 ms (gate 2 ms, met). The 2 MB case added 0.7 ms p99 (gate
5 ms, met); it is dominated by buffering the body once and the bounded JSON
probe. P3, P4 and P6–P9 are not automated yet; P7 (disconnect propagation)
is covered functionally by `client_disconnect_closes_upstream_promptly`
(< 250 ms bound with a 20 ms chunk cadence), not benchmarked.
