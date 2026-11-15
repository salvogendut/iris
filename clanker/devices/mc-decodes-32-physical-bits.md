# The Indy/Indigo2 fold high physical bits; O2 PCI is an exception

Status: rule, corrected 2026-11-15 for O2.

The original O2 generalization was incorrect: MACE has wide PCI windows.
See [O2 wide PCI decode](o2-wide-pci-must-not-alias-ram.md). The observations
below concern the IP28 stack; O2 retains legacy folding outside its wide I/O
interval, but high PCI addresses are decoded before folding and RAM aliasing.

**Symptom.** After the bus and `TranslateResult.phys` went 64-bit, the IP28
(R10000) guest flooded stderr with `MC: CPU Error at 20404000` and never got
past early kernel start. With the full address printed the errors were at
`0x6_2040_44d4`: the kernel stack, with physical bits 33-34 set.

**Cause.** IRIX's page-table entries keep software bits (the PFN lock bit
`0x10000000` among them) just above the PFN. The kernel writes the PTE to
EntryLo as is, so those bits land in the top of the PFN field and come out
as physical address bits 32 and up. `tlb dump` shows it: wired entry 0, the
kernel stack, has PFN `0x620404`. The R4400 and R10000 drive those bits on
SysAD, but the MC (and CRIME, as we model it) decodes the low 32 and the
board aliases the rest away. The old `u32` physical address modelled that by
accident.

**Rule.** Fold, don't widen: `MipsExecutor::phys_mask` is the board's decode
width, applied in `fold_phys` (after `phys_alias`) to mapped, xkphys and
ERL-identity translations. `machine.rs` and `ip32_machine.rs` set it to
`u32::MAX`; the default (all ones) is for tests and a future Octane. Folding
in the CPU rather than in `Physical` keeps the caches, the ppmem window and
the JIT's nutlb on the same 32-bit addresses, so a page like the kernel
stack does not fall onto the slow bus path.

**Also.** `tools/mon.py` talks to port 8888 unless `IRIS_MON_PORT` says
otherwise. When a second iris runs on another `monitor_port`, set it, or
`stop` lands on the wrong machine.
