# PROM boot presets must use each model's persistent environment format

Setting loader names in Prismatic configuration alone does not affect firmware.
`prom_env::prepare_boot_environment` is an offline, worker-only embedding API:
call under the state lease before constructing a machine. Keep existing means
no call. It atomically replaces one file after validating the full change.

- IP24 DS1386 file: 8192 bytes, 256-byte environment at byte 64, revision 8.
- IP22/IP28 EEPROM: 256 bytes, revisions 8/9. Loader at 52 (18 bytes), filename
  at 70 (28), options at 98 (12). Checksum starts at 0xa5, XORs each word's
  bytes except byte 0, rotating once per word. Preserve MAC and other fields.
- IP32: 512 KiB flash; page-aligned SHDR env segment, 64-byte header,
  NUL-separated name=value records and a zero-sum 32-bit body checksum.
- IP30: 1 MiB flash; FPDS region at 0xf0000, 24-byte header with a folded
  16-bit checksum. Environment entries have type 0x0100, a halfword count,
  validity 0xffff and a NUL-terminated name=value payload. Invalidate only
  replaced variables, append changes, retain unrelated records. No-op calls
  must not append. Refuse insufficient free space rather than erasing records.

Format references: SGI IP22nvram.h, RACER/IP30nvram.h, RACER/sflash.h and
stand/arcs/include/flash.h. Implementation is original Rust; no SGI source
text is copied. Focused tests exercise bundled O2/Octane images, preservation,
checksums, repeated presets and invalid-image rejection.
