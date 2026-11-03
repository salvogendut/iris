# Cache native GPR values explicitly and RTC conversion once per tick

Status: integrated into `xiri`, 2026-11-03. `perf/sgiemu-port` is retired.
Both caches now require `[perf] sgiemu_optimizations = true` (default false).
Historical measurements below were made with the caches enabled. See
[sgiemu-policy-belongs-to-the-machine.md](sgiemu-policy-belongs-to-the-machine.md)
for policy propagation and validation of the toggle.

## Source and scope

Reviewed SGIEmu through `5870980`. Its main performance work is
`572186cb46bbcbacc992cd9051c6e9a50dd21d50` ("JIT R4400 verso x86-64").
IRIS base: `087d250`. This is a Rust/Cranelift adaptation of techniques,
not a transplant of SGIEmu's handwritten x86-64 emitter.

| SGIEmu technique | IRIS treatment |
|---|---|
| Native integer, memory and FPU instructions | Already implemented by jitv2; retain IRIS's exception and FPU semantics. |
| Hot guest GPRs in host registers | Port as write-through Cranelift SSA variables; let its allocator select physical registers. |
| Fast per-mode address translation and direct RAM access | Existing nuTLB/tcache and inline load/store paths provide this role. |
| Physically keyed blocks and invalidation on code writes | Existing physical page pool and generation counters provide this role, including DMA and multiple CPUs. |
| Same-page constant-target chaining | Already compiled as edges within the physical-page function. |
| Cross-page chaining and per-site JR/JALR caches | Not ported: SGIEmu's single-threaded code lifetime/generation protocol cannot be copied into IRIS's shared, asynchronous SMP pool. A separate design and measurements are needed. |
| Cached DS1386 calendar conversion | Ported to DS1x86, invalidated on guest and raw monitor writes, NVRAM load and snapshot restore. |
| Disk snapshots and profiler | IRIS already has overlays, snapshots and profiling; these are support infrastructure for the SGIEmu comparison. |
| `--mips` time scaling | Not a throughput optimization; retain IRIS's timing model. |

SGIEmu's published boot speedup compares its interpreter with a new JIT.
It is not a prediction of the gain over IRIS's existing JIT.

## GPR cache rule

The old emitters repeatedly load `MipsCore.gpr`. Atomic interrupt checks and
opaque Rust callouts prevent Cranelift's ordinary store/load forwarding from
retaining those values. Initialize SSA variables at compiled-function entry,
read those variables during native execution, and update both the variable
and `MipsCore` on each GPR write. Register zero remains a constant.

Every early exit, memory helper and exception still sees precise architectural
registers; there is no dirty-register spill protocol. Loop/back-edge and
branch-join values are handled by Cranelift's SSA construction. The shared load
helpers are special: they write their destination directly, so refresh that
variable **after** checking success. A fault must preserve the old register.

Disable caching for any region containing an interpreter fallback and for
developer/lockstep builds. These paths may inspect or change GPR state outside
the native emitters. Also exclude FPU regions: an initial all-native version
improved integer loops but repeatedly lost about 20–25% in transcendental and
Whetstone kernels on R4400. Page-wide GPR retention is unsuitable for those
call-heavy regions; a future selective cache needs separate measurements.
Any future helper that writes a GPR must update its SSA copy or force the
memory path. Do not relax interrupt sampling to get this win.

Persistent compiled-page keys already include the executable hash. No cache
format or snapshot format change is necessary.

## RTC cache rule

RTC reads used to repeat Gregorian calendar conversion even within one
centisecond. Keep the converted absolute centisecond alongside the registers,
and skip conversion when it matches. The cache is derived state and is never
serialized. Invalidate on every path that can change calendar bytes or their
time base, including `rtc w`; raw monitor writes retain their original raw
semantics rather than being turned into guest clock-setting operations.

Regression coverage includes leap-day rollover, same-tick polling, freeze and
resume, raw monitor edits, and snapshot restore.

## Measurement

Host: AMD Ryzen 9 PRO 8945HS, Linux x86-64. Rust nightly-2026-10-28,
release build with `lightning,jitv2`; default shadow cache. Both binaries use
the same dependencies, toolchain and benchmark payload. Baseline is saved
before source edits; comparisons alternate baseline/candidate order. No builds
or unit tests run concurrently with timed measurements.

Three full runs per binary and CPU, alternating order, produced these
geometric means of the 46 per-kernel median throughput ratios:

| CPU | Candidate / baseline | Change | Checked kernels |
|---|---:|---:|---|
| R4400 | 1.0767x | +7.7% | 40/40 matched on all six runs |
| R5000 | 1.0718x | +7.2% | 40/40 matched on all six runs |

The remaining six kernels are marked `UNCHECKED` by the suite. Selected
per-kernel ratios show both gains and regressions:

| Kernel | R4400 | R5000 |
|---|---:|---:|
| `int/alu` | 2.253x | 2.194x |
| `int/alu64` | 2.362x | 2.371x |
| `int/alu_ilp` | 1.310x | 1.316x |
| `int/dhrystone` | 1.029x | 1.003x |
| `codec/lz` | 1.691x | 1.394x |
| `codec/huffman` | 1.546x | 1.543x |
| `fpu/whetstone` | 0.932x | 0.998x |
| `fpu/transcend` | 0.981x | 0.997x |
| `mem/unaligned` | 0.892x | 0.883x |
| `mem/random` | 1.043x | 0.911x |
| `mem/latency_dram` | 0.945x | 0.934x |
| `sys/cache_flush` | 0.911x | 1.006x |

This is not a universal speedup: unaligned memory is about 11–12% slower,
and some other kernels also regress. Excluding FPU regions reduces the much
larger losses in the first experiment, but does not promise unchanged timing
for workloads that also execute integer regions. A selective register cache
and per-entry initialization are candidates for further work, not verified
improvements from this port.

Local raw JSON, all 46 per-kernel ratios and logs are under ignored
`target/sgiemu-port/results/full/`; the comparison script is
`target/sgiemu-port/compare.py`. The rejected all-native experiment is retained
separately under `target/sgiemu-port/results/all-native-experiment/`.

The isolated RTC polling test fell from **122.32 ns/read to 22.52 ns/read**
(5.43x throughput). Each sample performs five million `BusDevice::read8`
calls cycling through offsets 0–13, with TE enabled and `black_box` around
the address/result. Three alternating process pairs each produce three
samples; the comparison uses the median of nine samples per binary. The
benchmark source is locally saved as `target/sgiemu-port/rtc_poll.rs`.

Reproduce each full run with:

```sh
cargo build --release --locked --features lightning,jitv2 --bin iris-bench
target/release/iris-bench run --cpu r4400 --label candidate-r4400 --out results
target/release/iris-bench run --cpu r5000 --label candidate-r5000 --out results
```

Use three runs per CPU and binary, then compare per-kernel median `work/ns`.
The cross-kernel summary is the geometric mean of those throughput ratios;
aggregate MIPS weights kernels differently as their autoscaled work changes.
Bare-metal throughput and an isolated RTC polling microbenchmark are not
measurements of IRIX boot or desktop responsiveness.

## Validation

- `cargo test --workspace --features jitv2 --locked --offline -- --test-threads=4`
  with `RUST_MIN_STACK=67108864`: 2,639 passed, 35 ignored, no failures.
- Release build with `lightning,jitv2` passed.
- An additional `iris-bench run --quick --cpu r10000` run matched all 40
  checked kernels. This is a correctness smoke test, not an R10000 speed claim.
- The new GPR regression compares a store/load loop with the interpreter,
  including loop-carried values, a changing delay-slot register and a final
  fault that must retain the destination value and report the correct EPC.
  Separate child processes exercise regular callouts and actual shared
  destination-writing helpers, avoiding process-global toggle races.
- The RTC regressions cover centisecond rollover into leap day, freeze/write/
  resume, snapshot restoration, and raw monitor-write invalidation.
