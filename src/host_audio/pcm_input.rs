//! Optional PCM file or FIFO feeding HAL2 analog input.
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read};
use std::path::Path;

pub(crate) struct PcmInput {
    reader: BufReader<File>,
    frame: [u8; 4],
    used: usize,
    idle: bool,
}

impl PcmInput {
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut opts = OpenOptions::new();
        opts.read(true);
        // A FIFO writer may connect late or pause. Never wait for it on the
        // emulator timer thread, including when opening its read end.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.custom_flags(libc::O_NONBLOCK);
        }
        let file = opts.open(path)?;
        Ok(Self { reader: BufReader::new(file), frame: [0; 4], used: 0, idle: false })
    }

    pub fn begin_tick(&mut self) { self.idle = false; }

    pub fn frame(&mut self) -> [u16; 2] {
        if self.idle { return [0; 2]; }
        while self.used < self.frame.len() {
            match self.reader.read(&mut self.frame[self.used..]) {
                Ok(0) => { self.used = 0; self.idle = true; return [0; 2]; }
                Ok(n) => self.used += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => { self.idle = true; return [0; 2]; }
            }
        }
        self.used = 0;
        let [a, b, c, d] = self.frame;
        [u16::from_le_bytes([a, b]), u16::from_le_bytes([c, d])]
    }
}
