# An 8042 translation bit must translate Set 2 key events

NetBSD 11.0/sgimips enables controller configuration bit 6 and selects scan-code
Set 2. `pckbd` consequently expects the controller to return XT/Set 1 codes.
Before this fix, `ps2 status` showed `scancode_set=2 config=47`, but `push_kb`
returned raw Set 2 bytes. In Xorg, typing `Xiri_123` produced unrelated keys
and missing releases, followed by autorepeat. The mouse worked.

Route Set 2 key events through the existing Set 1 encoder while bit 6 is set.
Keep the selected keyboard set unchanged and leave command replies alone.
When translation is disabled, retain Set 2 make/F0/break encoding. Set 3 keeps
its existing per-key make/break and repeat behavior. No extra snapshot state
is needed: configuration and selected set are already saved.

MACE and IOC3 use `write_to` for direct device ports without an 8042. Clear the
translation bit on this path so their Set 2 input remains untranslated.
The controller tests cover ordinary keys, Shift, an extended arrow, releases,
translation disabled, and direct device ports. Live O2/Octane testing is separate.

The Xiri guest probe uses monitor `key` commands through this same PS/2 event
path, not XTEST or serial input. Its acceptance text is `Xiri_123` plus Return.
