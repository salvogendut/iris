# MOUNT v3 must advertise AUTH_SYS for NetBSD clients

NetBSD 11.0 `mount_nfs -3 -u -U` reached the built-in UDP MOUNT server but
returned `Authentication error`. `getnfsargs.c:xdr_fh` requires the requested
RPCAUTH_UNIX flavor in a non-empty server list. Advertising only AUTH_NULL
therefore rejects the mount before any NFS file operation.

The server already accepts and skips both credential bodies. Advertise
AUTH_SYS (1) and retain AUTH_NULL (0). The `mount_returns_root_handle` wire
test checks both flavors. This changes negotiation, not access enforcement:
the backing directory still uses synthetic ownership and mode attributes.
