# Windows sidecars need write-through rename, not Unix directory fsync

Prismatic native Windows CI reached raw disk writes, then failed with OS error 5
from CowNode::record_new/rewrite_record. CHD disk tests passed. The new sidecar
had already been written, synced and renamed; chd_disk::fsync_dir then used
File::open on a directory, which Windows rejects.

Keep the strict directory fsync on Unix. On Windows publish the already-synced
sidecar with MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH), checking its result.
Do not add COPY_ALLOWED (replacement must remain on the same filesystem), and
reject embedded NUL before calling the UTF-16 API. Do not merely ignore all
sidecar publication errors; guest acknowledgement depends on this record.

Regression: cow_disk::tests::sidecar_publication_creates_replaces_and_reports_failure,
plus Prismatic's raw disk write/flush/reopen test on its native Windows runner.
