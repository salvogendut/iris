# Embedding must not depend on a graphics device for machine telemetry

2026-11-14. Prismatic integration of IP27/IP35 platforms.

The default Workstation::cpu_counters derives one counter from get_display.
Serial-only Origin machines have no display, and NUMA graphics machines can
have additional node CPUs. IP27, IP35 and Fuel now return their actual CPU
counters directly; Octane and Tezro already enumerate their processors.

IP35 serial construction previously bypassed HostBackends. L1 and Tezro C/D
now have optional injected transports, and IOC A/B honor the existing ones.
Keep the console CI tee and per-channel logging when overriding transport.
The focused ip35::serial::frontend_tests test occupies the TCP port and checks
that every injected line still exchanges bytes and fans output into CI.

Workstation frontend_scsi_bus exposes stopped-machine storage attachment on
Fuel/Tezro/IP27. Callers hold the machine slot and must not call back into a
controller while holding its SCSI bus. These are embedding interfaces, not
changes to guest CPU or device behavior.

## Shared SN constructors (2026-11-17)

Upstream now implements CPU counters in `sn::Machine` for both node families;
keep that implementation instead of the deleted board-local overrides. The
borrowed storage hooks live there too, through `BoardIo.frontend_storage`.
Its provider keeps the original controller alive and returns the same bus
used by the guest and flush path. IP27 exposes two ISP1040 buses, Fuel/Tezro
two ISP12160 channels, and the I-brick its separate Fibre Channel loop.
The regression compares those references against `with_disks`.

IP35 serial ports now use the common `SerialLine` helper and own their CI
handles. Apply `serial_line_with_override` to that helper so Unix sockets,
TCP settings, logging and the supplied frontend transport all survive.

Headless Onyx construction belongs in `ip35_machine::node_specs/build_io`;
USB input is controlled by `attach_usb` in the shared I-brick builder. Do not
restore the deleted per-product constructor files.

Prismatic still names `config::FuelConsole` when selecting a host serial
channel. Keep it as a type alias to upstream's shared `Ip35Console`; do not
restore the duplicate enum. The consumer check builds Prismatic with both
JIT and wgpu enabled against this checkout.
