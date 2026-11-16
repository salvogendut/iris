# Headless embedding must detach peripherals, not just hide the renderer

Prismatic issue #2 requires a serial-only machine with no physical keyboard or
monitor. Legacy `headless` only suppressed Newport; GR2/IMPACT and several PS/2
or USB input paths remained present. Heliacal also intentionally keeps its Sun
keyboard present for firmware; do not copy that workaround to SGI machines.

`MachineConfig::prepare_headless` and `workstation::build_headless` provide an
explicit embedding path. They remove optional graphics, detach Fuel/IP35 USB
input, and unplug the shared PS/2 peripherals before execution. The controller
remains accessible, including self-test and loopback; keyboard/mouse commands
receive no ACK. Integrated O2 GBE remains part of the motherboard. Onyx 3000
can build its C/I-bricks without a graphics brick. Device wiring persists
through PS/2 reset and is separate from guest snapshot state.

Prismatic chooses the platform's serial line and programs legacy PROM console
settings using the existing leased firmware-state API. Both GUI and PTY output
must be broadcast from the injected backend, not read competitively from one
host descriptor. Optional APIs are detected from source, never branch names.
