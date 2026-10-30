# MC restore must republish the physical memory-bank map

NetBSD 11.0 on an R4400 Indy resumed snapshots in the original process but
hung in a fresh process. The saved MEMCFG0 was 0x7f207f40 while `phys map`
still showed 0x08000000–0x17ffffff as unmapped RAM. CPU state and cached
instructions resumed briefly, then exception handling ran against the wrong
physical mapping. Serial input accumulated without being serviced.

`MemoryController::load_state` restored register bytes but never called the
MEMCFG callback that publishes the decode to the board. In-place restore
hid this because the PROM had already mapped the banks. Writing the same
MEMCFG values through the monitor and reloading the same snapshot recovered
the guest shell, confirming the missing derived state.

After loading MC state, decode MEMCFG0/1, release MC and DMA locks, and invoke
the registered bank-map callback. The regression test uses a fresh MC and
checks both publication and the unlocked callback contract. RAM contents
are still restored by the machine snapshot loader after device state.
