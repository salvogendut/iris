//! A per-machine PCM handoff for frontends that own their audio backend.

use super::HostOut;
use rtrb::Consumer;
use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc,
};

/// Called on the device/worker thread when an output is opened. Implementations
/// should hand the stream to their playback thread and return promptly. There
/// is no fallback to CPAL when a frontend factory is installed, even on error.
pub trait OutputFactory: std::fmt::Debug + Send + Sync {
    fn open(&self, stream: PcmStream) -> Result<(), String>;
}

/// Bounded, interleaved signed-16-bit stereo at `sample_rate()`. Reading and
/// discarding commit complete stereo frames, so overflow cannot shift channels.
/// The frontend owns this consumer; the emulated device owns its producer.
pub struct PcmStream {
    consumer: Consumer<i16>,
    source: String,
}

impl PcmStream {
    pub fn sample_rate(&self) -> u32 {
        48_000
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn is_closed(&self) -> bool {
        self.consumer.is_abandoned()
    }
    pub fn pending_samples(&self) -> usize {
        self.consumer.slots() & !1
    }

    pub fn read(&mut self, samples: &mut [i16]) -> usize {
        let count = self.pending_samples().min(samples.len() & !1);
        if count != 0 {
            let chunk = self
                .consumer
                .read_chunk(count)
                .expect("single audio consumer");
            let (first, second) = chunk.as_slices();
            samples[..first.len()].copy_from_slice(first);
            samples[first.len()..count].copy_from_slice(second);
            chunk.commit_all();
        }
        count
    }

    /// Discard at most `samples`, rounded down to a whole stereo frame.
    pub fn discard(&mut self, samples: usize) {
        let count = self.pending_samples().min(samples & !1);
        if count != 0 {
            self.consumer
                .read_chunk(count)
                .expect("single audio consumer")
                .commit_all();
        }
    }
}

pub(super) fn open(
    factory: &dyn OutputFactory,
    underruns: Arc<AtomicU64>,
    playing: Arc<AtomicBool>,
    ring_samples: impl Fn(u32) -> usize,
    who: &str,
) -> Option<HostOut> {
    // No more than 100 ms can collect while a frontend is not consuming.
    let (producer, consumer) = rtrb::RingBuffer::new(ring_samples(48_000).clamp(2, 9_600) & !1);
    let stream = PcmStream {
        consumer,
        source: who.to_owned(),
    };
    if let Err(error) = factory.open(stream) {
        crate::diagnostics::stderr(format_args!("{who}: frontend audio unavailable: {error}"));
        return None;
    }
    Some(HostOut {
        stream_rate: 48_000,
        producer,
        underruns,
        playing,
        _stream: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AudioConfig;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Capture(Mutex<Option<PcmStream>>);
    impl std::fmt::Debug for Capture {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("Capture")
        }
    }
    impl OutputFactory for Capture {
        fn open(&self, stream: PcmStream) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(stream);
            Ok(())
        }
    }

    #[test]
    fn frontend_output_is_bounded_preserves_channels_and_reports_close() {
        let factory = Arc::new(Capture::default());
        let audio = AudioConfig {
            output: Some(factory.clone()),
            ..AudioConfig::default()
        };
        let mut out = crate::host_audio::open_output(
            &audio,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(true)),
            |_| usize::MAX,
            "test",
            "IRIS_TEST_NO_CAPTURE",
        )
        .unwrap();
        let mut stream = factory.0.lock().unwrap().take().unwrap();
        assert_eq!(out.producer.slots(), 9_600);
        for _ in 0..4_800 {
            out.producer.push(123).unwrap();
            out.producer.push(-456).unwrap();
        }
        assert!(out.producer.push(999).is_err());
        let mut samples = [0; 3];
        assert_eq!(stream.read(&mut samples), 2);
        assert_eq!(samples, [123, -456, 0]);
        // Wrap the ring, then discard an odd request without losing L/R order.
        out.producer.push(123).unwrap();
        out.producer.push(-456).unwrap();
        stream.discard(3);
        assert_eq!(stream.read(&mut samples), 2);
        assert_eq!(samples, [123, -456, 0]);
        stream.discard(usize::MAX);
        assert_eq!(stream.pending_samples(), 0);
        assert!(!stream.is_closed());
        drop(out);
        assert!(stream.is_closed());
    }

    #[derive(Debug)]
    struct Refuse;
    impl OutputFactory for Refuse {
        fn open(&self, _: PcmStream) -> Result<(), String> {
            Err("no device".into())
        }
    }

    #[test]
    fn refused_frontend_does_not_open_cpal_and_factory_is_not_serialized() {
        let config = AudioConfig {
            output: Some(Arc::new(Refuse)),
            ..AudioConfig::default()
        };
        assert!(crate::host_audio::open_output(
            &config,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(false)),
            |_| 100,
            "test",
            "IRIS_TEST_NO_CAPTURE"
        )
        .is_none());
        let text = toml::to_string(&config).unwrap();
        assert!(!text.contains("output"));
        let loaded: AudioConfig = toml::from_str(&text).unwrap();
        assert!(loaded.output.is_none());
    }
}
