# Embedded guest power-off state

Kind: design. Updated: 2026-10-31.

Prismatic retained the last IRIX shutdown framebuffer and labelled every stopped
CPU "Machine halted". Guest power-off already reached `MachineEvent::PowerOff`;
with process exit disabled, it stopped devices but left the host worker alive.
A screenshot ending mid-shutdown therefore does not establish a guest hang.

The shared event controller now sets `GuestState::PoweredOff` after `stop()`
under the machine-slot lock. Repeated cleanup stops preserve it on IP22/IP28,
O2 and Octane. Reset clears it; `start_guest` resets before restarting a powered-
off machine. Monitor status serializes it as `powered_off`. The default standalone
exit path remains unchanged.

Hosts should distinguish this state from CPU inactivity (debugger breakpoints,
monitor stops), finish their own media cleanup and release handles before
announcing completed power-off. Prismatic publishes a separate powered-off phase,
clears the last framebuffer and allows Save to start a fresh machine.

A diskless IP22 regression can exercise the real path with monitor
`machine-stop`, then `mw 0xbfbd9850 0 b` (IOC panel bit 0 is active-low).
The first must remain resumable; the second must finish the host worker and save
motherboard EEPROM. Use temporary machine state and `IRIS_REX_JIT_PROFILE` in
`/tmp`. This verifies the emulated power latch, not a full IRIX shutdown.
