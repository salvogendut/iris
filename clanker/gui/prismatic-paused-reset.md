# Paused Indy reset can strand the PROM

Kind: open. Observed: 2026-11-01, core a40801f.

A diskless Indy (R4400, 64 MB, bundled PROM, default cache) displayed normally
on its initial boot through Prismatic. The sequence `stop`, `reset`, `stop`,
`start_guest` left the CPU running at `0xffffffffbfc177dc` at each 10-second
sample for 40 seconds; captured frames remained black. The extra stop after
reset kept the machine paused until the host resumed it. CPU status alone
therefore did not establish successful reset: check resumed guest pixels too.

One suspect is REX3 stop/start idempotence: `Rex3::stop` pushes `GFIFO_EXIT` even
when no processor thread remains to consume it. A later processor can consume
that leftover exit immediately. Confirm and cover the repeated-stop case before
changing the device; the PROM loop itself has not been disassembled here.

Prismatic F5 performs a cold reset through normal stop/flush/drop and a new
worker. A previously running machine boots again; a paused machine stays in
unstarted PAUSED configuration until Ctrl+P. This recreates CPU, device and
renderer state together. Ordinary Ctrl+P pause/resume preserves the worker;
repeated pause requests are ignored once paused. All investigation used fresh
temporary firmware state and no guest disks.
