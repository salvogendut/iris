# IP28 PROM bank layouts in an embedded frontend

Validated on 2026-10-31 using Prismatic and Iris integration branch
`xiri/netbsd-newport` at `888c4bd` (merged upstream `ad89773`). Tests used the
bundled `PROM0701477002`, R10000, Solid IMPACT, fresh temporary NVRAM/EEPROM,
no disks, and the interpreter build. No CPU/device behavior was changed.

Single-bank `[32, 0, 0, 0]` and `[128, 0, 0, 0]` pass configuration validation
but do not reach the graphics console. POST eventually clears MEMCFG0/1 and
loops at `0xffffffff9fc01964`–`0xffffffff9fc019a4`, toggling the front LED. The
128 MB result repeats with optimized release code, so this is not simply a
slow POST. The underlying sizing/rank-model cause remains unproven.

Equivalent layouts `[16, 16, 0, 0]` and `[64, 64, 0, 0]` reach the graphical
PROM and stop cleanly. The 128 MB input smoke enters the PROM monitor and
`hinv` reports R10000/195 MHz, 128 Mbytes, Solid Impact. The documented
`[512, 512, 0, 0]` layout also reaches the diskless graphical boot error and
stops cleanly.

Prismatic therefore fills IP28 presets from 16/64/256/512 MB banks. This keeps
the requested totals (384 MB becomes 256+64+64) without accepting the failing
32/128 MB bank layouts. Iris's GUI currently greedily uses all valid bank
sizes, including 32/128; its configuration validation alone does not prove
PROM compatibility. Other totals/layouts still require live guest validation.

Reproduce via Prismatic's README IP28 `--boot-smoke-test` / `--input-smoke-test`
commands, setting `IRIS_REX_JIT_PROFILE` under `/tmp`. The smoke tests isolate
preferences and machine state and reject attached disks. A nonuniform frame
is only a display check; inspect the captured PROM text for inventory evidence.
