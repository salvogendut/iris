# Remote services and performance overrides must follow machine lifetime

Kind: rule. Confirmed while embedding xiri in Prismatic (2026-11-01).

A detached GDB listener retained the old CPU and port across label/reset changes.
VNC Drop signalled stop but did not join its threads; a partially handshaken or
blocked client could retain display/input references. Use owned GdbServer and
VncServer handles, shut client sockets down and join all workers before dropping
the workstation. Keep socket publication synchronized with listener shutdown.

Thread affinity used OnceLock, so only the first machine's settings took effect.
init now replaces the configuration. Frontends must call it before every model's
constructor, not only Machine::new; query allowed processor IDs on an unpinned
thread and never use available_parallelism as an ID bound.

Do not call Jitv2Config::apply_env in a running threaded frontend. Configure the
persistent cache via pcache::configure between machines. Jobs must capture their
destination before enqueueing, or old writes can land in another label's cache.
The queue is bounded and pruning ignores directories without a build ID/marker.

Regression coverage: Prismatic real workers exchange GDB RSP, leave VNC mid
handshake, stop, rebind both ports and start another machine. Core tests check
replacement affinity and cache destinations/pruning.

The first VNC frame could be a correctly sized but all-black readback: enabling
capture repaints asynchronously, while grab_loop immediately copied the old rgba
buffer. On the first watching transition request/wait for a fresh frame (bounded
500 ms for paused machines) before publishing it. Prismatic's isolated release
check requests Raw pixels and verifies the diskless PROM image has multiple colors.

Perfetto lifecycle cleanup is idempotent via stop_if_running. Keep the session
mutex until its writer joins: a concurrent frontend stop must wait for an earlier
monitor stop to finish before releasing the CPU's guest-cycle source.
