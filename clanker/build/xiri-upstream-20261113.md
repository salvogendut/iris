# Xiri integration of upstream f0914a0b

Status: integration cycle, 2026-11-13.

The merge brings 645 upstream commits into fork parent
b23f674c235b37c3ac83808085d28ac128bb79c4. Upstream tip is
f0914a0b73d086f4b7a58cb8e362e2ce8973617a. Keep pinned lab worktrees and
qualified older executables unchanged.

Resolution rules:

- Use CPU Part/Family and per-model VPN masks; retain the fork's full
  32-bit VPN/region comparison required by the NetBSD MIPS64 kernel.
- Keep Newport RGB24 physical-plane encoding and partial-mask regressions.
- Use upstream selective loop GPR caching behind the fork's per-machine
  sgiemu_optimizations flag. Default off; developer/lockstep bypass it.
  Set the compile-worker option before spawning it: querying its owner
  mutex afterward deadlocks with the worker's lifetime lock. Fingerprint
  the option and check restart ownership. Keep RTC caching behind the flag.
- Use packed release PROMs and the shared resolver, including explicit
  override failures. Empty string remains an embedded alias for existing
  frontends. Pad O2/Octane flash to capacity. The new Octane release payload
  has no FPDS: first boot with Keep existing must initialize it. Do not
  assume that embedded firmware contains a previous machine's environment.
- Use upstream network Session/Running ownership. Preserve live forwarding
  configuration on renewal and drop old sockets before rebinding.
- Preserve frontend PCM, serial and diagnostics hooks with the newer host
  audio and serial interfaces. Prismatic requires source-detected adapters
  for CPU names, embedded firmware and network ownership.

Validation evidence is in Xiri's .work/shutdown-sessions. The full workspace
suite completes, including 5,482 library tests and interpreter/JIT Newport
RGB24 checks. The R4400 JIT bare-metal suite reports all 277 tests and exactly
the 15 named upstream failing tests (49 individual checks), with no new
failure or newly passing baseline entry. It was built with the available
NetBSD GCC 14.4.0 cross compiler rather than CI's GCC 16.2.1; it is correctness
coverage, not a comparable performance measurement.

Prismatic: 180 tests pass; source detection compiles against isolated
unmodified upstream main; both cores pass frontend clippy. The rebuilt
Instigator passes BOOTP/TFTP on Unix and TCP stream transports. Dummy SDL
and diskless graphical PROM smoke checks also pass.

The initial failed unit tests assumed old padded full-flash dumps and a
reserved cache-tag bit. The fixtures now seed unrelated O2 settings and a
synthetic initialized Octane PDS, check fresh-PDS refusal, and use an
architecturally meaningful cache-tag bit. Stream tests use packets longer
than Ethernet padding so packet framing is independent of upstream trimming.

The merged optimized core also boots the selected NetBSD/Xorg CHD with its
existing kernel/module. Original 4Dwm, catalog, xclock and winterm start.
An active-session shutdown -r saves before xdm stops; the next login restores
two winterms and xclock at the saved geometry, keeps the closed catalog
closed, and opens native shells as xiritest in /usr/people/xiritest.
The manual image's emulator remains independently pinned at 819c682.
