# Preserve embedded lifecycle support when merging upstream machine changes

Current workflow (2026-11-03): at the user's request, all performance-branch
work is now integrated into `xiri` via `26771f9`. Develop only on `xiri`;
`perf/sgiemu-port` is retired, not deleted. The root checkout now owns `xiri`;
the former integration worktree is clean and detached at `26771f9`, and the
NetBSD qualification worktree remains pinned. The separate-branch instructions
below describe the earlier upstream-only update, not the current workflow.
SGIEmu GPR/RTC caching is opt-in and off by default; see
[the per-machine policy note](../perf/sgiemu-policy-belongs-to-the-machine.md).

2026-11-02 integration of upstream `43e525a` into Xiri.

The fork loads Octane firmware through an embedded firmware provider, so upstream diagnostics cannot assume a local `prom_path` exists. Describe the loaded PROM revision instead. The initial merged build exposed that reference; both workspace and JIT/idle suites pass after correcting it.

Keep `Link` worker ownership/join state when adding upstream network reporters. Keep the fork reset path that transitions a powered-off machine to halted and restarts peripherals; share the upstream power-on body so new Odyssey/VPro state is reset too. Preserve both upstream headless snapshot limits and the fork paused-frontend snapshot provenance documentation.

Guest baselines stay pinned until separately qualified. Host tests for this integration: 2,578 workspace passes (33 ignored), 2,921 JIT/idle library passes (32 ignored). Nested child-process test reports are excluded from totals.

## 2026-11-03: storage, configuration and virtio serial

Upstream `68f4e4a6d56d85e109ef3d4363854df1cd39cfda` adds 16 commits after
`2eb811f`: block-device sizing, CHD zeroing/durability, serial ECS/stdout logs,
virtio serial, idle parking, NFS execute permissions, temporary CI overlays,
fallible config loading, `--machine` and `--validate`.

`xiri` integrated it as `c84757d` from `819c682` in its existing worktree.
That worktree already had the merge in progress when the performance-branch
update started; preserve its work rather than starting a competing merge.
Its recorded checks passed: workspace 2,668 tests/doctests (36 ignored), 25
idle-parking checks, CLI builds and configuration checks.

`perf/sgiemu-port` integrates the same upstream revision independently, from
`6dc5676`. Do not merge the two fork branches merely to update upstream.
Keep the GPR/RTC caches and the per-pool JIT priority queue fix unchanged.
Adopt the tested `xiri` flash-validation helper and virtio fixture correction:
Octane fixtures must select an R10000, and O2 tests should check the actual
unsupported-transport error. Extend upstream's centralized CPU acceptance
check to retain the performance branch's R12000/R14000 support only on Octane.
Both constructors retain `firmware::load_flash_prom`; `--validate` accepts
embedded/default firmware but rejects missing explicit flash overrides.

The Octane PROM smoke example previously expected IRIX's combined CPU/cache
inventory line. PROM actually prints `Processor: 600 Mhz R14000, with FPU`
and `Secondary cache size: 2 Mbytes` separately. Check those fields separately;
the original mismatch was a harness error after a successful boot and `hinv`.

Performance-branch validation uses nightly-2026-10-28, locked offline Cargo,
`jitv2,idle-pause,pcap,perfetto`, and two test threads. The container has
`libpcap.so.1` but lacks the development linker symlink; a local ignored
`target/host-libs/libpcap.so` plus `LIBRARY_PATH` suffices without modifying
system packages. CLI smoke artifacts and disposable Octane state are kept in
ignored `target/upstream-20261103/`.

Final performance-branch checks pass: workspace 3,243 tests/doctests (45 ignored,
excluding duplicate subprocess summaries), all nine CLI cases, and the corrected
diskless R14000/V12 PROM `hinv` check. The JIT restart queue regressions pass;
the GPR/RTC/queue implementation files are identical to pre-merge `6dc5676`.
