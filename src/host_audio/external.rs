//! Per-machine frontend PCM output, backed by the shared iris-audio renderer.

use iris_audio::{Backend, DeviceFormat, Error, OutputRequest, Render, RenderInfo, StreamHandle};
use rtrb::Consumer;
use std::sync::{
    atomic::{AtomicBool, Ordering::Relaxed},
    Arc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Hand the consumer to the frontend's playback thread and return promptly.
/// A refusal never falls back to another host audio backend.
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
    /// Create a 100 ms stereo queue for an embedded renderer or a frontend
    /// fixture. The producer must enqueue interleaved pairs of samples.
    pub fn channel(source: impl Into<String>) -> (rtrb::Producer<i16>, Self) {
        let (producer, consumer) = rtrb::RingBuffer::new(9_600);
        (
            producer,
            Self {
                consumer,
                source: source.into(),
            },
        )
    }

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

pub(super) struct FrontendBackend(pub Arc<dyn OutputFactory>);

struct FrontendStream {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StreamHandle for FrontendStream {
    fn device(&self) -> String {
        "frontend PCM".into()
    }
}

impl Drop for FrontendStream {
    fn drop(&mut self) {
        self.stop.store(true, Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Backend for FrontendBackend {
    fn name(&self) -> &'static str {
        "frontend"
    }

    fn open_output(
        &self,
        request: &OutputRequest,
        mut render: Box<dyn Render>,
    ) -> Result<Box<dyn StreamHandle>, Error> {
        // Preserve the frontend's fixed stereo interface and 100 ms bound.
        // iris-audio handles device-rate changes, mixing and channel mapping.
        let (mut producer, stream) = PcmStream::channel(request.name.clone());
        self.0.open(stream).map_err(|message| Error::Backend {
            backend: "frontend",
            message,
        })?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread =
            std::thread::Builder::new()
                .name("frontend PCM".into())
                .spawn(move || {
                    render.configure(DeviceFormat {
                        rate: 48_000,
                        channels: 2,
                    });
                    let start = Instant::now();
                    let mut played = 0u64;
                    let mut samples = [0.0f32; 9_600];
                    while !stopped.load(Relaxed) && !producer.is_abandoned() {
                        std::thread::sleep(Duration::from_millis(5));
                        let due = (start.elapsed().as_nanos() * 48_000 / 1_000_000_000) as u64;
                        let frames = (due - played).min(4_800) as usize;
                        played = due;
                        if frames == 0 {
                            continue;
                        }
                        let info = RenderInfo {
                            device_delay: Some(Duration::from_secs_f64(
                                (9_600 - producer.slots()) as f64 / 96_000.0,
                            )),
                            device_underruns: 0,
                        };
                        render.render(&mut samples[..frames * 2], &info);
                        let count = (frames * 2).min(producer.slots() & !1);
                        if count != 0 {
                            producer
                                .write_chunk_uninit(count)
                                .expect("single PCM producer")
                                .fill_from_iter(samples[..count].iter().map(|&v| {
                                    (v * 32768.0).round().clamp(-32768.0, 32767.0) as i16
                                }));
                        }
                    }
                })?;
        Ok(Box::new(FrontendStream {
            stop,
            thread: Some(thread),
        }))
    }
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
    fn bounded_pcm_keeps_stereo_frames_through_wrap_and_discard() {
        let (mut producer, mut stream) = PcmStream::channel("test");
        for _ in 0..4_800 {
            producer.push(123).unwrap();
            producer.push(-456).unwrap();
        }
        assert!(producer.push(999).is_err());
        let mut samples = [0; 3];
        assert_eq!(stream.read(&mut samples), 2);
        assert_eq!(samples, [123, -456, 0]);
        producer.push(123).unwrap();
        producer.push(-456).unwrap();
        stream.discard(3);
        assert_eq!(stream.read(&mut samples), 2);
        assert_eq!(samples, [123, -456, 0]);
        stream.discard(usize::MAX);
        assert_eq!(stream.pending_samples(), 0);
        assert!(!stream.is_closed());
        drop(producer);
        assert!(stream.is_closed());
    }

    #[test]
    fn shared_renderer_reaches_frontend_and_last_voice_drop_closes_it() {
        let factory = Arc::new(Capture::default());
        let config = AudioConfig {
            output: Some(factory.clone()),
            prebuf_ms: 0,
            ..Default::default()
        };
        let mut voices = crate::host_audio::open_outputs(
            &config,
            "test",
            2,
            &[48_000, 24_000],
            "IRIS_TEST_NO_CAPTURE",
        )
        .unwrap();
        let mut stream = factory.0.lock().unwrap().take().unwrap();
        assert_eq!(stream.sample_rate(), 48_000);
        assert_eq!(stream.source(), "test");
        voices[0].write(&[4000i16, -8000].repeat(4000));
        voices[1].write(&[0i16, 0].repeat(2000));
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut heard = false;
        while Instant::now() < deadline && !heard {
            let mut samples = [0; 9600];
            let n = stream.read(&mut samples);
            heard = samples[..n]
                .chunks_exact(2)
                .any(|f| f[0] > 3000 && f[1] < -6000);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(heard, "rendered stereo must reach the frontend");
        voices.pop();
        assert!(!stream.is_closed());
        drop(voices);
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
    fn refusal_never_falls_back_and_factory_is_not_serialized() {
        let config = AudioConfig {
            output: Some(Arc::new(Refuse)),
            host: crate::config::AudioHost::Null,
            ..Default::default()
        };
        assert!(
            crate::host_audio::open_output(&config, "test", 2, 48_000, "IRIS_TEST_NO_CAPTURE")
                .is_none()
        );
        let text = toml::to_string(&config).unwrap();
        assert!(!text.contains("output"));
        assert!(toml::from_str::<AudioConfig>(&text)
            .unwrap()
            .output
            .is_none());
    }
}
