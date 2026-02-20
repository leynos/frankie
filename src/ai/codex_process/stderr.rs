//! Captured stderr output from a Codex child process.
//!
//! Spawns a background reader thread that drains the child's stderr
//! stream into a bounded buffer, making the captured text available
//! for inclusion in failure messages.

use std::io::BufRead;
use std::process::ChildStderr;
use std::sync::{Arc, Mutex};

/// Maximum number of bytes to capture from stderr (64 KiB).
const STDERR_LIMIT: usize = 65_536;

/// Captured stderr output from a child process.
///
/// Spawns a background thread that drains the child's stderr stream
/// into a bounded buffer. The captured text can later be appended to
/// failure messages via [`StderrCapture::append_to`].
pub(super) struct StderrCapture {
    buffer: Arc<Mutex<String>>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl StderrCapture {
    /// Starts capturing stderr from the child process.
    pub(super) fn spawn(child_stderr: Option<ChildStderr>) -> Self {
        let buffer = Arc::new(Mutex::new(String::new()));
        let reader_thread = child_stderr.map(|readable| {
            let handle = Arc::clone(&buffer);
            std::thread::spawn(move || Self::drain(readable, &handle))
        });
        Self {
            buffer,
            reader_thread,
        }
    }

    /// Reads lines from stderr into the shared buffer up to the size limit.
    fn drain(readable: ChildStderr, buffer: &Mutex<String>) {
        let reader = std::io::BufReader::new(readable);
        for result in reader.lines() {
            let Ok(text) = result else { break };
            let Ok(mut content) = buffer.lock() else {
                break;
            };
            if content.len() + text.len() + 1 > STDERR_LIMIT {
                break;
            }
            content.push_str(&text);
            content.push('\n');
        }
    }

    /// Appends any captured stderr to `message`, or returns it unchanged
    /// when stderr is empty. Joins the reader thread first to ensure all
    /// output has been collected.
    pub(super) fn append_to(&mut self, mut message: String) -> String {
        if let Some(thread) = self.reader_thread.take()
            && let Err(payload) = thread.join()
        {
            let panic_detail = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic payload".to_owned());
            message = format!("{message}\n\nstderr reader thread panicked: {panic_detail}");
        }

        let captured = self
            .buffer
            .lock()
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.clone());

        match captured {
            Some(text) => format!("{message}\n\nstderr:\n{text}"),
            None => message,
        }
    }
}
