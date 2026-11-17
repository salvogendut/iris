# Snapshot guards use the frontend's snapshot root

Kind: rule. Updated: 2026-11-17.

Embedded frontends set the Indy snapshot root with `set_snapshot_context`.
Upstream's resident snapshot guard initially checked `saves/<name>` while
the later restore used the configured root, so it could miss an incomplete
snapshot in the frontend's directory.

Resolve the directory once from `self.snapshot_root`, before the incomplete
marker check and before resetting devices. Use that path for every part of
the restore. `incomplete_snapshot_in_frontend_root_is_rejected` exercises
the guard with a temporary frontend directory and needs no hardware GPU.
