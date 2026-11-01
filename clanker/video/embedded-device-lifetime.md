# Embedded optional-device lifetime (2026-11-01)

VINO owns a source whose CdmcAdjustedSource owns a Vino clone; physical memory
also points back to mapped VINO. Stopping DMA alone does not release the camera.
Machine Drop must call shutdown to take both sources and sys_mem, dropping sources
outside their locks. DmaWake must latch a pending notification under its mutex:
an immediate stop can notify before the DMA worker waits, otherwise losing the
wakeup and hanging join. The same race affects guest DMA enable writes. `vino status` must call source
status outside VINO state/source locks: CdmcAdjustedSource reads CDMC registers
and re-enters the VINO state lock. Desktop monitor testing reproduced this deadlock. Linux capture retries must observe running=false and V4L2
stream dequeue must have a timeout, or camera Drop can block the worker indefinitely.
Ultra64 uses a fixed SHM name: obtain its persistent file lease before stale unlink.
Never unlink the lease sidecar on release (inode races). Older binaries without the
lease must not run an Ultra64 bridge concurrently.
Paused native snapshot load must not briefly restart peripheral DMA after loading.
Frontend checkpoint gates exclude unsaved device state and verify content hashes;
native DiskRef path/size checks alone cannot identify a same-size replacement base.

Physical keeps strong device references and other devices point back at Physical.
Map Ultra64 through its weak bus adapter so that graph cannot retain the board's
SHM and lease after Machine drop. EventImpl destructors touch SHM: event fields
must precede shmem, then the lease. Ultra64 Drop stops its listener first. Clear
stale listener signals before spawning, never inside the worker (lost stop wakeup).
An isolated SHM-name regression test covers 32 immediate start/stops, retained
bus adapter, board drop and reopen without touching the user's bridge.
