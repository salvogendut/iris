# Hashless CHDs need a sparse overlay instead of a MAME child

Symptom: a valid uncompressed v5 hard-disk CHD passes read-only inspection,
but creating a COW child fails with `InvalidFile`, leaving a `.diff.chd`.

Cause: uncompressed CHDs may have a zero SHA-1. `HdImage::open_with_diff`
creates a child with that zero parent hash, then reopens it with a parent.
MAME rejects a supplied parent when the child header has no parent hash.

`block_node::open_chd_cow` selects a sparse `CowNode` over a privately owned,
read-only CHD handle for this case. Its wrapper rejects commit (raw offsets
must never reach the CHD container), preserves writes on flush/reopen, and
supports rollback. Hashed bases still use `ChdNode` and MAME diffs.

Only a valid v5 uncompressed legacy child with matching geometry and an
entirely unallocated hunk map can be bypassed. Read the map in bounded chunks;
allocated all-zero data is a write, not an empty image. Never delete or mutate
the old child. Invalid/nonempty children must remain a visible recovery error.

Prismatic uses this helper; callers using `ChdHd::open` or `ChdNode::open`
directly retain their existing behavior. Prismatic's media tests cover both
paths, persistence, rollback, commit rejection and legacy-child failure cases.
