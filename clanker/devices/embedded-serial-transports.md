# Embedded serial endpoints need explicit revocation on unload

Prismatic PTY lifecycle tests found that an Indy endpoint remained open after
the worker stopped and dropped its Workstation: device graph references can
keep a backend Arc alive. Do not rely on the final Arc drop for frontend host
resources. Prismatic wraps each transport in a revocable handle and clears it
after stopping the worker's machine, including startup failures and unwinding.
Retained callbacks become inert, and old terminal clients receive hangup.

`MachineConfig.serial_backends` is runtime-only, skipped by serde. A/B match
existing config channels: console is B on IP2x and A on IP30/IP32. An override
replaces TCP and Unix endpoints but keeps serial log tees and CI fanout. No
override preserves standalone constructors. Host callbacks must not block UART
threads. Prismatic uses safe nix PTYs, raw slave settings, nonblocking masters,
a held slave for reconnects, and a bounded 64 KiB output queue with loss counts.
Both endpoints must open before either is installed. Preserve drafts: allocate
only on the machine worker after Save, never while editing or validating.

## Performance-build restart limitation (2026-11-02)

Diskless Indy with 128 MB and `console=d`: boot to System Maintenance Menu on
PTY B, save a switch to a fixed TCP port, confirm restart, then Ctrl+P. The
`pcap,idle-pause,perfetto,jitv2` release build (without developer) opens the TCP
endpoint but PROM panics with a UTLB miss near PC `a8747cd8`. Reproduced with both
Keep-existing and NetBSD boot preferences. The default-feature interpreter
passes the same UI script. Quitting and launching the performance binary with
the same saved NVRAM and TCP configuration reaches the PROM menu. The cause is
not yet isolated; do not infer that TCP binding or PTY teardown failed.

All five profiles pass fresh performance-build PTY boots, console input, pause,
reconnect and quit/hangup checks. Configure routes before initial boot or
relaunch the frontend after changing an already-booted machine.
