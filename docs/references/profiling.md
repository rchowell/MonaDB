# Rust Profiling

This is a guide for CPU profiling, memory analysis, and system-level tracing. Nanoseconds and cache behavior matter. Using the "profiling" build profile.

## CPU Profiling

Use samply for firefox-comptaible profiling and flamegraph for SVGs.

```bash
cargo install sampy
cargo install flamegraph
```

**Examples**

```bash
samply record cargo run --profile profiling -- [args]
# opens a browser

cargo flamegraph --profile profiling -- [args]
# outputs flamegraph.svg
```

## Memory Profiling

- Use dhat
- heaptrack, and bytehound for memory profiling.

```toml
[dev-dependencies]
dhat = "0.3"        # in-process allocation analysis
```

**Example**

This produces a `dhat-heap.json` file which you can open in the [DHAT viewer](https://nnethercote.github.io/dh_view/dh_view.html).

```rust
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    let _profiler = dhat::Profiler::new_heap();
    // run workload
    // profile written to dhat-heap.json on drop
}
```

Open `dhat-heap.json` . It shows total bytes allocated and live bytes per call site — useful for finding which operations cause heap churn you want to eliminate.

### heaptrack — full heap timeline

No code changes required. Uses `LD_PRELOAD` to intercept `malloc`/`free`.

```bash
cargo build --profile

heaptrack ./target/release/your_binary [args]

heaptrack --analyze heaptrack.your_binary.*.zst
```

The GUI shows allocation timeline, peak live memory, allocation hotspots, and flamegraphs. Good for memory growth investigations that dhat-rs doesn't fully explain.

### bytehound — Rust-native heap profiler with web UI

More setup than heaptrack but better for Rust-specific workflows. Supports jemalloc via `jemallocator`, can stream data to a remote machine, and has a web-based GUI with a Rhai scripting DSL for automated analysis.

```bash
LD_PRELOAD=./libbytehound.so MEMORY_PROFILER_LOG=warn ./your_binary
```


## Microbench

### iai-callgrind — cache-aware instruction counting

The right tool for reliably detecting small regressions. Runs each benchmark exactly once under Valgrind's Callgrind, so results are deterministic across machines and CI environments.

```toml
# Cargo.toml
[dev-dependencies]
iai-callgrind = "0.14"
iai-callgrind-runner = "0.14"
```

Example output:

```
insert::bench_insert_100k
  Instructions:     1,234,567 | 1,250,000  (+1.25%)
  L1 Hits:          1,100,000 | 1,090,000  (-0.91%)
  RAM Hits:             2,100 |     3,400  (+61.90%)  ← regression
  Estimated Cycles: 1,500,000 | 1,680,000  (+12.00%)
```

**RAM Hits is the number to watch for storage engines.** A spike there means you've broken a cache-friendly access pattern — far more diagnostic than wall-clock time alone.

You can configure regression thresholds to fail CI if a metric exceeds a limit:

```rust
LibraryBenchmarkConfig::default()
    .regression(RegressionConfig::default()
        .limits([EventKind::EstimatedCycles, 5.0]))
```

iai-callgrind also integrates DHAT and Massif, so you can run heap profiling from the same benchmark suite.

### divan — wall-clock benchmarks

A lighter alternative to criterion with a clean table output. Use alongside iai-callgrind: iai catches regressions in CI, divan confirms they translate to real wall-clock time.

```toml
[dev-dependencies]
divan = "0.1"
```

```
                fastest  │  slowest  │   median │     mean
 insert_100k    1.533 µs │ 118.4 µs  │ 2.171 µs │ 2.344 µs
 lookup_key     997.7 ns │  36.98 µs │ 2.383 µs │ 2.476 µs
```

## Memory Profiling

---

## System-level tracing with bpftrace

`bpftrace` is the right tool for questions at the OS boundary — when you need to see what's happening below the allocator or the VFS layer. No code changes, no restart required.

### Large allocation tracing

Useful for catching unexpected mmap calls from jemalloc arenas expanding behind your back:

```bash
bpftrace -e '
tracepoint:syscalls:sys_enter_mmap
/args->len > 67108864 && pid == $1/
{
    printf("%s: mmap %d MB\n", comm, args->len / 1024 / 1024);
    ustack();
}' $(pgrep your_db)
```

### fsync latency histogram

```bash
bpftrace -e '
kprobe:do_fsync              { @start[tid] = nsecs; }
kretprobe:do_fsync /@start[tid]/ {
    @fsync_lat_us = hist((nsecs - @start[tid]) / 1000);
    delete(@start[tid]);
}'
```

### Page fault rate by call site

Reveals working set pressure and hot paths that are pushing data out of the page cache:

```bash
bpftrace -e '
software:page-faults:1 /pid == $1/ {
    @[ustack()] = count();
}' $(pgrep your_db)
```

---

## eBPF continuous profiling

### Parca — always-on CPU profiling with differential flamegraphs

Useful when you need a time-indexed record of where CPU was spent — "what changed between the v1.2 and v1.3 binary?" Samples at 19Hz system-wide via eBPF with minimal overhead. Differential flamegraphs highlight exactly which functions consumed more or less CPU between two profiles.

```bash
# run parca server, then:
parca-agent --node=dev --store-address=localhost:7070
```

Point your browser at `localhost:7070` and query by time range or git SHA.

---

## Workflow

Use these tools in this order to avoid wasting time optimizing in the wrong layer:

1. **iai-callgrind** — get a deterministic baseline. Add to CI with regression limits on Estimated Cycles and RAM Hits.
2. **samply** — explore a real workload visually. Confirm iai's findings are real hotspots, not Callgrind simulation artifacts.
3. **dhat-rs** — audit allocation patterns. For a storage engine, zero-allocation hot paths are often achievable and worth pursuing.
4. **bpftrace** — investigate OS-boundary anomalies: unexpected page faults, fsync spikes, or jemalloc mmap explosions.
5. **bytehound or heaptrack** — only if dhat-rs leaves a memory question unanswered. Heavier tools, but the GUI timeline catches growth patterns that per-callsite analysis misses.

> **Note on iai-callgrind vs wall-clock:** Callgrind simulates a CPU, so absolute cycle counts won't match real latency. Use iai for regression *detection* and samply/divan for understanding *actual* hot paths.
