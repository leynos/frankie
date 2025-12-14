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
        let Ok(serialised) = serde_json::to_string(&event) else {
            return;
        };

        let _ignored = writeln_stderr(&serialised);
    }
}

fn writeln_stderr(message: &str) -> io::Result<()> {
    use io::Write;

    let mut stderr = io::stderr().lock();
    writeln!(stderr, "{message}")
}

#[cfg(test)]
mod tests {
    use super::{TelemetryEvent, TelemetrySink};

    #[derive(Debug, Default)]
    struct RecordingSink {
        events: std::sync::Mutex<Vec<TelemetryEvent>>,
    }

    impl RecordingSink {
        fn take(&self) -> Vec<TelemetryEvent> {
            self.events
                .lock()
                .expect("events mutex should be available")
                .drain(..)
                .collect()
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
}
