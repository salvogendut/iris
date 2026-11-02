# Both compilation queues belong to their JIT pool

Status: fixed on `perf/sgiemu-port`, 2026-11-02.

## Symptom

Prismatic's release build with `pcap,idle-pause,perfetto,jitv2` boots a diskless
128 MB Indy to the PROM menu on PTY B. Saving a switch to TCP, confirming the
restart and resuming produces a PROM panic instead of the menu. The original
report was a UTLB miss near `a8747cd8`; this verification captured a read-address
error at `a8747d34`, bad address `c5`, followed by the same
`PANIC: Unexpected exception`. The interpreter control (`IRIS_JIT_DISPATCH=0`)
passes. A fresh process also boots with the saved TCP configuration.

## Cause and rule

The ordinary compilation FIFO was per-pool, but `HOT_QUEUE` was process-global.
Embedded machine teardown can leave device-graph references and compiler
workers alive. An old worker could pop a replacement machine's priority
request, read its page through the old bus, and publish the resulting code into
the replacement's `PhysicalCodePage`. Stopping or flushing one pool could also
drain another pool's requests and clear its scheduling flags.

The global queue exists in both pre-fix `perf/sgiemu-port` (`59c2f16`) and
`xiri` (`e049373`); it is separate from the SGIEmu GPR/RTC cache changes. Runtime
reproduction used the performance branch and Prismatic, not a standalone
`xiri` binary.

Each `CompileQueue` now owns a bounded priority lane alongside its FIFO. CPUs
cache both handles, and `jitv2_share` rebinds both for SMP. Workers pop only
their own pool's queues; stop and flush drain those same two queues before
page memory can be released. Queue capacity, priority and drop-on-full behavior
are unchanged. Do not move requests, raw page pointers or compiled artifacts
between machine pools. This fixes ownership even when an old graph remains
alive; it does not solve the separate lifetime issue retaining that graph.

## Verification

- Saved pre-fix release binary: the exact PTY-to-TCP UI script reproduces
  `PANIC: Unexpected exception` on the second boot.
- Same binary, JIT dispatch disabled: the script passes.
- Fixed release binary, no developer instrumentation: passes with both
  Keep-existing and NetBSD PROM presets. These are diskless firmware checks,
  not guest OS boots. Cancellation keeps the old route; confirmation hangs up
  the old PTY, leaves the replacement paused, then boots to the TCP PROM menu
  after resume; quitting releases the port.
- `compile_worker_cannot_take_another_pools_hot_request`: a real worker must
  process its FIFO without consuming another pool's priority request. The
  request is then compiled and published by its own worker using a different
  bus. FIFO completion synchronizes the check, rather than a timed sleep.
- `stopping_one_pool_preserves_another_pools_hot_requests`: stopping one pool
  clears only its own page's scheduling flag; the other request remains queued.
- `cargo +nightly-2026-10-28 test --workspace --locked --offline --features jitv2
  -- --test-threads=2`: passed, 3,151 tests and doctests, 42 ignored (excluding
  duplicate summaries from subprocess checks). The core library accounts for
  3,028 passes and 39 ignored. This includes the GPR cache equivalence tests,
  shared memory helper subprocesses, RTC tests and multi-worker compiler tests.

Local reproduction artifacts are under ignored `target/jit-restart/`, including
saved before/after executables, scripts, screenshots and serial transcripts.
