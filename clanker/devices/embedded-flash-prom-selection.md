# Empty O2/Octane PROM paths select embedded bytes directly

The Prismatic fork embeds O2 4.18 (`prombinip32`) and Octane 4.9
(`prombinip30`). Both arrays are byte-identical to the user-supplied files;
their source filenames, sizes and SHA-1 hashes are in the generated headers.
`python3 tools/embed-prom.py INPUT OUTPUT SYMBOL` reproduces either array.

`firmware::load_flash_prom` distinguishes three cases:

- Empty `prom`: embedded bytes, with no working-directory probes. Prismatic
  uses this when no external override is selected.
- `DEFAULT_PROM`: try `prom.bin`, then the model's legacy filename, then
  embedded bytes only if both files are absent.
- Other explicit paths: read that file or report the error. Never hide a typo,
  unreadable file or failed menu override by substituting another ROM.

Existing persistent flash remains authoritative, as before: O2's `nvram` and
Octane's `octane.flash` may contain a previous image. Use fresh isolated state
for firmware smoke tests; do not delete a user's flash merely to exercise the
fallback. This is different from Indigo2's legacy fallback, which still probes
its local model filename even when the configured path is empty.

Validation: both loader tests pass, and generated arrays match every source
byte and the recorded hashes. Prismatic diskless checks at 128 MB reach the
O2 boot-error prompt with R5000/R10000. Octane 4.9 reaches the PROM menu after
Enter acknowledges internal/external SCSI device/cable diagnostic failures.
The early nonuniform-frame smoke capture only shows its power-on diagnostics;
a longer desktop run was needed to inspect POST and the menu. No guest OS was
booted and no user disks or persistent state were used.
