//! Application telemetry events and sinks.
//!
//! Frankie is a local-first tool, but it still benefits from lightweight
//! telemetry to support debugging and to capture operational signals such as
//! the active database schema version.

use std::io;

use serde::{Deserialize, Serialize};

/// A structured telemetry event emitted by Frankie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    /// Records the current database schema version after migrations apply.
    SchemaVersionRecorded {
        /// Diesel migration version string (e.g. `20251220000000`).
        schema_version: String,
    },
    /// Records the latency of a sync operation.
    SyncLatencyRecorded {
        /// Duration of the sync operation in milliseconds.
        latency_ms: u64,
        /// Number of comments fetched.
        comment_count: usize,
        /// Whether the sync was incremental (merged) or full refresh.
        incremental: bool,
    },
}

/// A sink that can record telemetry events.
pub trait TelemetrySink: Send + Sync {
    /// Records a telemetry event.
    fn record(&self, event: TelemetryEvent);
}

/// Telemetry sink that drops all events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTelemetrySink;

impl TelemetrySink for NoopTelemetrySink {
    fn record(&self, _event: TelemetryEvent) {}
}

/// Records telemetry events to stderr as JSON lines (JSONL).
///
/// This is intended for local debugging and is not transmitted anywhere.
#[derive(Debug, Default)]
pub struct StderrJsonlTelemetrySink;

impl TelemetrySink for StderrJsonlTelemetrySink {
    fn record(&self, event: TelemetryEvent) {
        let line = serde_json::to_string(&event)
            .unwrap_or_else(|_| r#"{"type":"telemetry_serialisation_failed"}"#.to_owned());

        // Stderr write failures are intentionally ignored; there's no
        // meaningful recovery action for local telemetry.
        drop(writeln_stderr(&line));
    }
}

fn writeln_stderr(message: &str) -> io::Result<()> {
    use io::Write;

    let mut stderr = io::stderr().lock();
    writeln!(stderr, "{message}")
}

/// Test support utilities for telemetry testing.
#[cfg(feature = "test-support")]
pub mod test_support {
    use std::sync::{Arc, Mutex};

    use super::{TelemetryEvent, TelemetrySink};

    /// An in-memory telemetry sink that captures events for later assertion.
    #[derive(Clone, Default)]
    pub struct RecordingTelemetrySink {
        events: Arc<Mutex<Vec<TelemetryEvent>>>,
    }

    impl RecordingTelemetrySink {
        /// Returns a snapshot of all recorded events without draining.
        ///
        /// Use this when you need to inspect events without clearing them,
        /// such as when multiple Then steps need to check the same events.
        ///
        /// # Panics
        ///
        /// Panics if the events mutex is poisoned, which indicates a prior panic
        /// during event recording.
        #[expect(
            clippy::expect_used,
            reason = "test fixture; panic is acceptable if mutex is poisoned"
        )]
        #[must_use]
        pub fn events(&self) -> Vec<TelemetryEvent> {
            self.events
                .lock()
                .expect("events mutex should be available")
                .clone()
        }
    }

    impl TelemetrySink for RecordingTelemetrySink {
        #[expect(
            clippy::expect_used,
            reason = "test fixture; panic is acceptable if mutex is poisoned"
        )]
        fn record(&self, event: TelemetryEvent) {
            self.events
                .lock()
                .expect("events mutex should be available")
                .push(event);
        }
    }
}

/// Unit tests for telemetry module.
#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::test_support::RecordingTelemetrySink;
    use super::{TelemetryEvent, TelemetrySink};

    #[test]
    fn recording_sink_captures_events() {
        let sink = RecordingTelemetrySink::default();

        sink.record(TelemetryEvent::SchemaVersionRecorded {
            schema_version: "20251220000000".to_owned(),
        });

        assert_eq!(
            sink.events(),
            vec![TelemetryEvent::SchemaVersionRecorded {
                schema_version: "20251220000000".to_owned(),
            }]
        );
    }

    #[test]
    #[expect(
        clippy::indexing_slicing,
        reason = "Test asserts length before indexing"
    )]
    fn recording_sink_captures_sync_latency_events() {
        let sink = RecordingTelemetrySink::default();

        sink.record(TelemetryEvent::SyncLatencyRecorded {
            latency_ms: 150,
            comment_count: 42,
            incremental: true,
        });

        let events = sink.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            TelemetryEvent::SyncLatencyRecorded {
                latency_ms: 150,
                comment_count: 42,
                incremental: true,
            }
        );
    }
}
