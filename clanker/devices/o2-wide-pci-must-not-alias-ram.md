# O2 wide PCI addresses must not alias RAM

Status: rule, 2026-11-15.

The NetBSD N64 O2 kernel stopped with RI at `memset+0x4c`, a valid `sd`,
during AIC-7880 attachment. The store first took a normal XTLB miss. The
refill vectors were correct at `ahc_pci_attach` entry but corrupted after
controller setup: physical 0x84 changed from 0x00000040 to 0x04c000e0.
No disks or vendor payload were required to reproduce this.

The CPU's 32-bit physical mask folded MACE PCI memory at 0x280000000 to
0x80000000; the no-ECC alias then sent it to RAM. Controller register
writes changed exception-vector bytes. The old high-window mapping and
BAR runs also used truncated addresses, and high PCI I/O was missing.

MACE specification section 9.3 (pp. 159–160) places PCI I/O at
0x100000000–0x1ffffffff and PCI memory at 0x280000000–0x2ffffffff; the
intervening 2 GB are reserved. Preserve that interval before RAM folding,
map both windows at full width, and dispatch wide windows before narrowing
onboard register offsets. Keep the low windows and byte lanes unchanged.

Regressions cover CPU translation, the no-ECC alias, reserved addresses,
high/low I/O byte lanes, memory byte lanes and published BAR mappings.
The native O2 boot fixture and firmware diagnostic live in
`../NetBSD-xiri/xiri/platforms`; qualification evidence belongs to `../xiri`.

Validation: 14 MACE PCI tests and 213 CPU executor tests pass. The original
XIRI_O2 N64 kernel reaches root selection diskless; with a private copy of
the NetBSD CHD it discovers sd0, mounts FFS read-only, runs uname/mount in
single-user mode, and halts cleanly to the PROM. The exception vectors stay
intact. Original input hashes are unchanged. The Octane ARCS64 diagnostic
also passes on the candidate emulator. These are interpreter tests, not
hardware or IRIX desktop qualification; existing qualified images stay pinned.
