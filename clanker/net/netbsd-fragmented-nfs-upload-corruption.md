# NetBSD fragmented NFS uploads can corrupt data after restore

Status: open, observed during Xiri snapshot qualification on 2026-10-30 at
`d642079`, R4400/full cache, `tlbvmap`, NetBSD 11.0. Default 8192-byte UDP NFS
requests were used; the guest unmounted successfully after the copy.

One second-restore upload of 4 MiB failed its host SHA-256 comparison. Six
512-byte sectors differed in two regions. At destination offsets 337920 and
1378304, sampled bytes matched the source 1480 bytes later, exactly an IPv4
fragment payload at MTU 1500. This suggests fragment/DMA-buffer handling but
is not a confirmed root cause. Other repeated restores passed the same
transfer. Cold-boot I/O tests had also passed earlier; snapshot dependence
has not been established.

Do not explain this as duplicate RPC replies without packet evidence: the
corrupted regions contain displaced data, not simply missing WRITE blocks.
Keep host-side checksum verification. Xiri retains `--nfs-size 8192` as the
fragmented regression workload and separately qualifies `-r 1024 -w 1024`.
HTTP is another independent transfer path. The MC bank-remap snapshot fix
addresses a separate fresh-process failure and does not repair this finding.
