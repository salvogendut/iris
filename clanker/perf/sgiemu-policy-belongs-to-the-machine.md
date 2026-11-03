# SGIEmu cache policy belongs to each machine and its persistent code key

Status: implemented on `xiri`, 2026-11-03; the performance branch is retired.

## Symptom and cause

The original port always enabled the GPR cache in eligible JIT regions and the
DS1x86 calendar cache. A frontend preference alone cannot disable either. A
process-global flag would also let old/new machines interfere during a restart,
and a code-generation flag missing from the persistent fingerprint would allow
on-mode code to be loaded while the setting is off.

## Rule

`MachineConfig.perf.sgiemu_optimizations` defaults to false, including old TOML.
Each machine builder passes it through `MipsCpuConfig` to its `Jitv2` pool. The
inline compiler and every background worker copy the same setting. Arena resets
preserve it, worker restarts read it again, and SMP siblings share their pool.
`Codegen::cache_fingerprint` includes it even when a region does not use caching.
No production environment variable controls this policy.

Indy/Indigo2 configure DS1x86 under its existing mutex before CPU startup. Off
means conversion on every clock update. Writes and restores still invalidate
the cached conversion. The host policy is deliberately absent from snapshots
and NVRAM. O2/Octane use other RTC implementations.

Prismatic saves `advanced.expert.sgiemu_optimizations` per machine label and
detects the optional core API from source. Older cores preserve the preference
but disable the control. Save/Cancel/Discard reuse its normal restart flow;
the default is off even in JIT builds. iris-gui exposes the same core setting.
Developer/lockstep and FPU/fallback exclusions remain in place. The per-pool hot
queue fix and Newport plane-decoding fix are unconditional.

## Validation

- Workspace with `jitv2,idle-pause,pcap,perfetto`: 3,249 passed, 45 ignored;
  nested subprocess reports excluded. Includes the off-mode equivalence suite.
- `IRIS_TEST_SGIEMU=1` JIT equivalence suite: 259 passed, 2 ignored. This variable
  is read only by the test harness. The memory-helper/fault regression explicitly
  runs on/off with inline memory and shared helpers in four child processes.
- Focused checks cover default migration, TOML roundtrip, worker recreation,
  arena reset, distinct persistent fingerprints, RTC rollover, writes, freeze
  and restore in both modes.
- Prismatic: 157 production-feature tests, 155 default-feature tests; 126 tests
  against the isolated older `main` core (unavailable control and preserved
  preference). Strict frontend Clippy and formatting pass.
- A disposable diskless Indy, in one Prismatic process: off → on → off through
  F9 Performance; cancel then confirm each restart, verify saved setting and old
  PTY hangup, resume to the PROM menu all three times, quit cleanly. Compilation
  counters confirm JIT activity in each mode. Artifacts: `target/sgiemu-toggle/`.
- Dummy SDL and graphical diskless PROM smoke checks pass with caching off and
  on. The Newport physical-plane regressions pass in both generic and `rex-jit`
  renderers (1,044 draws per path).

These are host correctness and PROM lifecycle checks, not a new OS boot-time
benchmark or a full IRIX/NetBSD guest qualification.
