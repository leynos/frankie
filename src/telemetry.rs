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
        /// Diesel migration version string (e.g. `20251214000000`).
        schema_version: String,
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
        match serde_json::to_string(&event) {
            Ok(serialised) => {
                // Stderr write failures are intentionally ignored; there's no
                // meaningful recovery action for local telemetry.
                drop(writeln_stderr(&serialised));
            }
            Err(error) => {
                let fallback = format!(
                    r#"{{"type":"telemetry_serialisation_failed","error":{}}}"#,
                    serde_json::to_string(&error.to_string())
                        .unwrap_or_else(|_| "\"unknown\"".to_owned())
                );
                // Stderr write failures are intentionally ignored; there's no
                // meaningful recovery action for local telemetry.
                drop(writeln_stderr(&fallback));
            }
        }
    }
}

fn writeln_stderr(message: &str) -> io::Result<()> {
    use io::Write;

    let mut stderr = io::stderr().lock();
    writeln!(stderr, "{message}")
}

/// Test support utilities for telemetry testing.
///
/// This module provides a shared [`RecordingSink`] implementation that can be
/// used across unit tests, integration tests, and BDD scenarios.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    #![allow(
        clippy::expect_used,
        clippy::missing_panics_doc,
        reason = "test-only code; panics are acceptable in test fixtures"
    )]

    use std::sync::{Arc, Mutex};

    use super::{TelemetryEvent, TelemetrySink};

    /// A telemetry sink that records events for later assertion.
    ///
    /// This sink is thread-safe and can be cloned to share across test
    /// boundaries. Use [`take`](Self::take) to drain and retrieve recorded
    /// events, or [`events`](Self::events) for a non-draining snapshot.
    #[derive(Debug, Clone, Default)]
    pub struct RecordingSink {
        events: Arc<Mutex<Vec<TelemetryEvent>>>,
    }

    impl RecordingSink {
        /// Drains and returns all recorded events.
        ///
        /// This clears the internal event list. Subsequent calls return an
        /// empty vector until new events are recorded.
        #[must_use]
        pub fn take(&self) -> Vec<TelemetryEvent> {
            self.events
                .lock()
                .expect("events mutex should be available")
                .drain(..)
                .collect()
        }

        /// Returns a snapshot of all recorded events without draining.
        ///
        /// Use this when you need to inspect events without clearing them,
        /// such as when multiple Then steps need to check the same events.
        #[must_use]
        pub fn events(&self) -> Vec<TelemetryEvent> {
            self.events
                .lock()
                .expect("events mutex should be available")
                .clone()
        }
    }

    impl TelemetrySink for RecordingSink {
        fn record(&self, event: TelemetryEvent) {
            self.events
                .lock()
                .expect("events mutex should be available")
                .push(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::RecordingSink;
    use super::{TelemetryEvent, TelemetrySink};

    #[test]
    fn recording_sink_captures_events() {
        let sink = RecordingSink::default();
        sink.record(TelemetryEvent::SchemaVersionRecorded {
            schema_version: "20251214000000".to_owned(),
        });

        assert_eq!(
            sink.take(),
            vec![TelemetryEvent::SchemaVersionRecorded {
                schema_version: "20251214000000".to_owned(),
            }]
        );
    }

    #[test]
    fn recording_sink_events_returns_snapshot_without_draining() {
        let sink = RecordingSink::default();
        sink.record(TelemetryEvent::SchemaVersionRecorded {
            schema_version: "20251214000000".to_owned(),
        });

        let first = sink.events();
        let second = sink.events();

        assert_eq!(first, second, "events() should not drain");
        assert_eq!(first.len(), 1);
    }
}
