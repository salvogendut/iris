# R5000 Config IB/DB are read-only

NetBSD/sgimips on an R5000 Indy stopped at `r5k picache line size 16` before
serial login. The CPU model initialized Config for 32-byte lines, but shared
`MipsCore::write_cp0` let a later write clear IB/DB. NEC VR5000 User Manual
section 6.5.8, printed pp.129-131, identifies both fields as fixed/read-only.

Preserve Config bits 5:4 for PRId implementation 0x23, including the Indy's
revision 1.0 and O2's revision 2.1. Keep K0 writable and leave R4400 writes
unchanged. The executor regression issues MTC0 Config against both models;
it fails before the correction. No cache data-path or byte-order change.
