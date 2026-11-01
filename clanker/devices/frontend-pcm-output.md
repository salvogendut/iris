# Embedded audio factories belong to individual machines

Prismatic uses SDL3, while standalone Iris uses CPAL. Setting `no_audio`
disabled the emulated HAL2 device rather than providing a host mute. Keep
guest devices enabled and install `AudioConfig.output` before construction.

The runtime-only field is an `Arc<dyn host_audio::OutputFactory>`, skipped by
serde and cloned into the machine's audio devices. `open_output` hands the
factory a `PcmStream` and never tries CPAL if the factory was installed, even
if it refuses the stream. This covers HAL2, MACE and RAD with no process-global
hook, so machines and tests cannot steal each other's audio.

The handoff is 48 kHz signed-16-bit interleaved stereo. The lock-free ring is
capped at 100 ms; its consumer commits whole stereo frames. A frontend must
keep consumption bounded, discard stale samples when paused/muted, and drop
its host stream when the producer closes or the active machine changes. HAL2
opens a new output after stop/start; MACE/RAD may keep their output allocated.
The factory runs on a device/worker thread, possibly under a device lock: only
hand off ownership there, and do host window/audio work on the frontend thread.

Prismatic retains one pending stream, drains at most 40 ms per UI tick and
caps SDL's queue at 60 ms. Mute/volume are shared desktop preferences and apply
only after Save; errors report once and retry on mute/unmute or a new machine.
The screenshot shutter remains a separate SDL stream.

A controlled HAL2 DMA tone verified Indy/IP22 output through SDL's disk
backend: stereo peaks 12000/6000, then 6000/3000 at saved 50% volume, silence
while muted, no draft side effects, and successful pause/resume, cold reset
and label switching. IP28/O2 PROM audio also produced non-silent SDL captures.
The HAL2 status trace exposed AES TX leaving an unbounded digital-loopback
queue while RX was idle; it is now capped at 96,000 words with oldest-first
discard. This does not add host digital-audio input/output.
