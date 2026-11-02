# Preserve embedded lifecycle support when merging upstream machine changes

2026-11-02 integration of upstream `43e525a` into Xiri.

The fork loads Octane firmware through an embedded firmware provider, so upstream diagnostics cannot assume a local `prom_path` exists. Describe the loaded PROM revision instead. The initial merged build exposed that reference; both workspace and JIT/idle suites pass after correcting it.

Keep `Link` worker ownership/join state when adding upstream network reporters. Keep the fork reset path that transitions a powered-off machine to halted and restarts peripherals; share the upstream power-on body so new Odyssey/VPro state is reset too. Preserve both upstream headless snapshot limits and the fork paused-frontend snapshot provenance documentation.

Guest baselines stay pinned until separately qualified. Host tests for this integration: 2,578 workspace passes (33 ignored), 2,921 JIT/idle library passes (32 ignored). Nested child-process test reports are excluded from totals.
