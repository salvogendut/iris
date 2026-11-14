# Embedded firmware selection uses the upstream loader

Use upstream `dev::embedded_prom`, `dev::prom_image` and `iris-prom`.
`data/proms/proms.toml` records source files and hashes; `data/proms/*.deflate`
contains the decoded, packed payloads. The old generated Rust arrays and their
converter are obsolete. See [the upstream packing notes](embedded-prom-compression.md).

The remaining fork behavior is the empty-string alias for `"(embedded)"`.
Prismatic uses it when no external override is selected. Both spellings select
embedded bytes without probing the working directory. The default PROM name
still probes local defaults; other explicit paths must load or return an error.

Existing flash state remains authoritative. O2 and Octane pad the upstream
release payload to flash capacity; the Octane release payload has no initialized
persistent-data store. Let the PROM initialize fresh flash before applying an
offline boot preset. Do not erase a user's flash to exercise a firmware fallback.

Validation is through upstream's packed-payload hash/banner and resolver tests,
plus the fork's `flash_prom_validation_preserves_embedded_and_explicit_selection`
check. Earlier checks of full-flash Rust arrays are historical, not evidence for
the current packed payloads.
