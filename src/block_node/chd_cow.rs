//! Copy-on-write CHD disks, including uncompressed bases without a SHA-1.
use super::{BlockNode, BlockStatus, ChdNode, CowNode, check_range};
use libchdman_rs::Chd;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

fn chd_error(error: impl std::fmt::Debug) -> io::Error {
    io::Error::other(format!("{error:?}"))
}

/// Open a CHD hard disk with a persistent overlay and a read-only base.
/// Hashed bases use MAME diffs. Hashless uncompressed bases use a sparse sector
/// overlay because MAME cannot reopen a child whose parent's SHA-1 is zero.
/// The sparse overlay supports rollback, but deliberately refuses commit: raw
/// sector offsets must never be applied directly to a CHD container.
pub fn open_chd_cow(path: &str) -> io::Result<Box<dyn BlockNode>> {
    let chd = Chd::open(path, false, None).map_err(chd_error)?;
    if chd.sha1() != [0; 20] {
        drop(chd);
        return Ok(Box::new(ChdNode::open(path, true)?));
    }
    let info = chd.info().map_err(chd_error)?;
    let geometry = libchdman_rs::hd::read_geometry(&chd).map_err(chd_error)?;
    if info.compressed
        || geometry.sector_bytes != 512
        || info.unit_bytes != 512
        || info.logical_bytes == 0
        || !info.logical_bytes.is_multiple_of(512)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Hashless CHD overlays require an uncompressed hard disk with 512-byte sectors",
        ));
    }
    let diff = crate::chd_disk::diff_path_for(Path::new(path));
    // Old versions created a child with a zero parent hash, then failed to
    // reopen it. Bypass only a demonstrably empty child; never hide guest data.
    if diff.exists() {
        check_empty_legacy_diff(&diff, &chd).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "Cannot bypass existing CHD diff {}: {error}; preserve it for recovery",
                    diff.display()
                ),
            )
        })?;
    }
    let mut overlay = diff.into_os_string();
    overlay.push(".overlay");
    let inner = CowNode::new(
        Some(Box::new(ReadOnlyChd(chd))),
        PathBuf::from(overlay),
        info.logical_bytes,
    )?;
    Ok(Box::new(SparseChd {
        inner,
        base: PathBuf::from(path),
    }))
}

/// Check the v5 uncompressed hunk map, not the data bytes: an allocated hunk
/// full of zeroes is still a guest write and must not be ignored.
fn check_empty_legacy_diff(path: &Path, parent: &Chd) -> io::Result<()> {
    let diff = Chd::open(
        path.to_str()
            .ok_or_else(|| io::Error::other("Non-UTF-8 CHD path"))?,
        false,
        None,
    )
    .map_err(chd_error)?;
    let info = diff.info().map_err(chd_error)?;
    if info.version != 5
        || info.compressed
        || info.sha1 != [0; 20]
        || info.parent_sha1 != [0; 20]
        || info.logical_bytes != parent.logical_bytes()
        || info.hunk_bytes != parent.hunk_bytes()
        || info.unit_bytes != parent.unit_bytes()
        || libchdman_rs::hd::read_geometry(&diff).map_err(chd_error)?
            != libchdman_rs::hd::read_geometry(parent).map_err(chd_error)?
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an empty legacy diff for this disk",
        ));
    }
    let mut file = File::open(path)?;
    let mut header = [0; 124];
    file.read_exact(&mut header)?;
    let map_offset = u64::from_be_bytes(header[40..48].try_into().unwrap());
    let mut remaining = u64::from(info.hunk_count) * 4;
    if map_offset < 124
        || map_offset
            .checked_add(remaining)
            .is_none_or(|end| end > file.metadata().map(|m| m.len()).unwrap_or(0))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid CHD hunk map",
        ));
    }
    file.seek(SeekFrom::Start(map_offset))?;
    let mut buffer = [0; 65536];
    while remaining != 0 {
        let count = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..count])?;
        if buffer[..count].iter().any(|&b| b != 0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "diff contains allocated hunks",
            ));
        }
        remaining -= count as u64;
    }
    Ok(())
}

struct ReadOnlyChd(Chd);
// Like ChdHd, this owns a MAME handle exclusively. Ownership may move to the
// SCSI thread, but the handle is never shared or accessed concurrently.
unsafe impl Send for ReadOnlyChd {}

impl BlockNode for ReadOnlyChd {
    fn size(&self) -> u64 {
        self.0.logical_bytes()
    }
    fn read(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        check_range(self.size(), offset, buf.len() as u64)?;
        self.0.read_bytes(offset, buf).map_err(chd_error)
    }
    fn write(&mut self, _: u64, _: &[u8]) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "CHD base is read-only",
        ))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn discard(&mut self, _: u64, _: u64) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "CHD base is read-only",
        ))
    }
    fn block_status(&self, offset: u64, len: u64) -> io::Result<BlockStatus> {
        check_range(self.size(), offset, len)?;
        Ok(BlockStatus::data(len))
    }
    fn backing(&self) -> Option<&dyn BlockNode> {
        None
    }
}

struct SparseChd {
    inner: CowNode,
    base: PathBuf,
}
impl BlockNode for SparseChd {
    fn size(&self) -> u64 {
        self.inner.size()
    }
    fn read(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read(offset, buf)
    }
    fn write(&mut self, offset: u64, buf: &[u8]) -> io::Result<()> {
        self.inner.write(offset, buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
    fn discard(&mut self, offset: u64, len: u64) -> io::Result<()> {
        self.inner.discard(offset, len)
    }
    fn block_status(&self, offset: u64, len: u64) -> io::Result<BlockStatus> {
        self.inner.block_status(offset, len)
    }
    fn backing(&self) -> Option<&dyn BlockNode> {
        self.inner.backing()
    }
    fn dirty_count(&self) -> usize {
        self.inner.dirty_count()
    }
    fn is_cow(&self) -> bool {
        true
    }
    fn overlay_paths(&self) -> Option<(PathBuf, PathBuf)> {
        Some((self.base.clone(), self.inner.overlay_path().to_owned()))
    }
    fn commit(&mut self) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Committing a sparse CHD overlay is not supported; keep the overlay with the base",
        ))
    }
    fn rollback(&mut self) -> io::Result<()> {
        self.inner.rollback()
    }
    fn export_overlay(&mut self, dest: &Path) -> io::Result<Vec<u64>> {
        self.inner.export_overlay(dest)
    }
    fn import_overlay(&mut self, source: &Path, dirty: Vec<u64>) -> io::Result<()> {
        self.inner.import_overlay(source, dirty)
    }
}
