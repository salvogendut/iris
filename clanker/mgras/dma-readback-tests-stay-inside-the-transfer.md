# DMA readback parity tests must stay inside the armed transfer

Status: 2026-11-22, rule, `wgpu/tests.rs::transfer_reads_see_the_drawing`.

The fixed-seed (`0x4EAD`) GPU parity test failed at step 64, read line 4,
with six differing bytes in a 42-byte line. The failure reproduced with
unmodified upstream code and with the Xiri integration on Vulkan/RADV.

The test programmed a block from `(42, 38)` to `(71, 40)`: three rows.
It then chose a separate random number of DMA lines, up to eight, without
limiting that number to the block height. Line 4 was outside the armed
transfer. `gpu_sync_transfer` refreshes the CPU mirror only under that
transfer's rectangle; the CPU reference renderer has no stale mirror.
Reading an unsynchronized row therefore does not establish a drawing bug.
A diagnostic full `gpu_sync(Observation::Validation)` before rereading
the failing line made its bytes match the reference exactly.

Limit the randomized read count to `transfer_shape().0`, retaining the
random draw so later test steps keep the same seed sequence. Keep the
existing checks that more than 500 lines were read and that actual GPU
readback occurred. All 13 IMPACT GPU tests passed with the bounded reads.
The final readback test also passed separately on RADV/Radeon 780M and
Lavapipe software Vulkan; neither run skipped GPU initialization.
No raster, shader, or production readback change is needed for this failure.

Do not replace partial readback with a full-memory synchronization to make
an out-of-transfer test read pass: that hides the invalid test operation
and defeats the selective-readback design.
