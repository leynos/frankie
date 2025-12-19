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
#[cfg_attr(feature = "test-support", mockall::automock)]
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
/// This module re-exports the mockall-generated [`MockTelemetrySink`] and
/// provides a [`CapturingMockSink`] adapter for scenarios that need to capture
/// events for later inspection.
#[cfg(feature = "test-support")]
pub mod test_support {
    use std::sync::{Arc, Mutex};

    pub use super::MockTelemetrySink;
    use super::{TelemetryEvent, TelemetrySink};

    /// A mockall-backed telemetry sink that captures events for later assertion.
    ///
    /// This adapter wraps [`MockTelemetrySink`] and stores recorded events in a
    /// thread-safe buffer. Use [`events`](Self::events) to retrieve a snapshot of
    /// captured events for assertion in BDD scenarios.
    #[derive(Clone)]
    pub struct CapturingMockSink {
        events: Arc<Mutex<Vec<TelemetryEvent>>>,
    }

    impl Default for CapturingMockSink {
        fn default() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl CapturingMockSink {
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

    impl TelemetrySink for CapturingMockSink {
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
///
/// These tests require the `test-support` feature to be enabled, which provides
/// the mockall-generated [`MockTelemetrySink`].
#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::test_support::MockTelemetrySink;
    use super::{TelemetryEvent, TelemetrySink};

    #[test]
    fn mock_sink_receives_expected_event() {
        let mut mock = MockTelemetrySink::new();
        mock.expect_record()
            .withf(|event| {
                matches!(
                    event,
                    TelemetryEvent::SchemaVersionRecorded { schema_version }
                    if schema_version == "20251214000000"
                )
            })
            .times(1)
            .return_const(());

        mock.record(TelemetryEvent::SchemaVersionRecorded {
            schema_version: "20251214000000".to_owned(),
        });
    }
}
