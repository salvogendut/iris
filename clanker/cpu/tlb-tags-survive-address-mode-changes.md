# TLB tags retain Region and the upper VPN in 32-bit mode

The LP64 NetBSD sgimips kernel uses xkseg while its installed o32 userland
runs with UX clear. A shell returning from a syscall fetched zeros and then
kernel pointer words instead of its text. The failing user PC was 0x2bb00.
The TLB contained a global mapping for 0xc00000000002a000; the user page
0x2a000 had no valid PTE yet and should have taken a refill.

`vcmp32` masked away both Region and VPN[39:32]. The vmap candidate check
and MRU scan therefore accepted the kernel entry for the user address.
Global affects the ASID check only; it does not remove the VPN/Region check.
The same issue affects extended xuseg addresses with nonzero VPN[39:32].

Retain `0xc00000ffffffe000` (subject to PageMask) in both comparison masks,
including snapshot reconstruction and TLBP. Valid 32-bit addresses are
sign-extended internally. Do not change refill-vector selection, which still
follows KX/SX/UX. The earlier XTLB-vector fix addressed the KX=1 direction;
its claim that a narrower comparison was valid with KX/UX=0 was incomplete.

`test_o32_cannot_alias_extended_tlb_entries` fails on the previous revision.
It checks fetch/read/write misses, TLBP, global and ASID-local entries,
colliding vmap candidates, upper VPN bits, and snapshot reconstruction.
The old tests that expected 32-bit cross-region hits encoded the bug and
now expect misses.

Reference: MIPS R4000 Microprocessor User's Manual, second edition, chapter 4,
figures 4-5/4-6 (internal sign extension) and EntryHi/TLB fields. The isolated
Xiri native64 lab records the guest-level reproduction and qualification.
