//! Optional routing of routine core diagnostics for embedded frontends.
//!
//! Without a sink, messages retain their usual stdout/stderr destination.
//! Host failures and explicit monitor/file tracing do not use this channel.
use std::{
    fmt,
    io::{self, Write},
    sync::{Arc, RwLock},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// Called on the emitting core thread, potentially while it holds device locks.
/// Implementations must be quick, thread-safe, and must not call into devices
/// or SDL. Copy into a bounded buffer if the frontend needs asynchronous output.
pub trait DiagnosticSink: Send + Sync {
    fn write(&self, stream: Stream, message: fmt::Arguments<'_>);
}

#[derive(Default)]
struct Router(RwLock<Option<Arc<dyn DiagnosticSink>>>);

impl Router {
    fn set_sink(&self, sink: Option<Arc<dyn DiagnosticSink>>) {
        // Drop the old sink outside the routing lock, just like callbacks.
        let old = std::mem::replace(&mut *self.0.write().unwrap(), sink);
        drop(old);
    }

    fn emit(&self, stream: Stream, message: fmt::Arguments<'_>) {
        let sink = self.0.read().unwrap().clone();
        if let Some(sink) = sink {
            sink.write(stream, message);
        } else {
            write_console(stream, message);
        }
    }
}

static ROUTER: Router = Router(RwLock::new(None));

/// Install a process-wide frontend sink, or restore console output with None.
/// An already-running callback may finish on the previous sink after replacement.
pub fn set_sink(sink: Option<Arc<dyn DiagnosticSink>>) {
    ROUTER.set_sink(sink);
}

/// Write directly to the original console stream, bypassing the installed sink.
pub fn write_console(stream: Stream, message: fmt::Arguments<'_>) {
    match stream {
        Stream::Stdout => {
            let _ = writeln!(io::stdout().lock(), "{message}");
        }
        Stream::Stderr => {
            let _ = writeln!(io::stderr().lock(), "{message}");
        }
    }
}

pub fn stdout(message: fmt::Arguments<'_>) {
    ROUTER.emit(Stream::Stdout, message);
}

pub fn stderr(message: fmt::Arguments<'_>) {
    ROUTER.emit(Stream::Stderr, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Capture(Mutex<Vec<(Stream, String)>>);
    impl DiagnosticSink for Capture {
        fn write(&self, stream: Stream, message: fmt::Arguments<'_>) {
            self.0.lock().unwrap().push((stream, message.to_string()));
        }
    }

    #[test]
    fn routes_threads_streams_and_sink_replacement() {
        let router = Arc::new(Router::default());
        let first = Arc::new(Capture(Mutex::new(Vec::new())));
        router.set_sink(Some(first.clone()));
        std::thread::scope(|scope| {
            for i in 0..4 {
                let router = &router;
                scope.spawn(move || router.emit(Stream::Stderr, format_args!("probe {i}")));
            }
        });
        assert_eq!(first.0.lock().unwrap().len(), 4);
        let second = Arc::new(Capture(Mutex::new(Vec::new())));
        router.set_sink(Some(second.clone()));
        router.emit(Stream::Stdout, format_args!("resolution {}", 1282));
        assert_eq!(
            *second.0.lock().unwrap(),
            vec![(Stream::Stdout, "resolution 1282".into())]
        );
        assert_eq!(first.0.lock().unwrap().len(), 4);
        router.set_sink(None);
        assert!(router.0.read().unwrap().is_none());
    }

    #[test]
    fn callback_can_change_routing_without_holding_the_router_lock() {
        struct Reset(Arc<Router>);
        impl DiagnosticSink for Reset {
            fn write(&self, _: Stream, _: fmt::Arguments<'_>) {
                self.0.set_sink(None);
            }
        }
        let router = Arc::new(Router::default());
        router.set_sink(Some(Arc::new(Reset(router.clone()))));
        router.emit(Stream::Stderr, format_args!("reset"));
        assert!(router.0.read().unwrap().is_none());
    }
}
