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
