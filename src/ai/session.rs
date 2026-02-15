//! Session state model and persistence for Codex execution sessions.
//!
//! Each Codex execution run creates a JSON sidecar file alongside its
//! transcript, recording metadata such as thread ID, status, and PR
//! context. This enables session discovery and resumption after
//! interruptions.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::github::IntakeError;

/// Status of a Codex execution session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is currently running.
    Running,
    /// Session completed successfully.
    Completed,
    /// Session was interrupted (e.g. process crash, signal, or
    /// `turn/completed` with status `interrupted`).
    Interrupted,
    /// Session failed (non-zero exit or protocol error).
    Failed,
    /// Session was cancelled by the user or server.
    Cancelled,
}

/// Persistent state for a Codex execution session.
///
/// Serialized as a JSON sidecar file alongside the transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
    /// Current status of this session.
    pub status: SessionStatus,
    /// Path to the transcript JSONL file.
    pub transcript_path: Utf8PathBuf,
    /// Server-side thread ID from `thread/start` response, used for
    /// `thread/resume`. Populated once the thread is created.
    pub thread_id: Option<String>,
    /// Repository owner (e.g. `"octocat"`).
    pub owner: String,
    /// Repository name (e.g. `"hello-world"`).
    pub repository: String,
    /// Pull request number.
    pub pr_number: u64,
    /// Timestamp when the session started.
    pub started_at: DateTime<Utc>,
    /// Timestamp when the session reached a terminal state.
    pub finished_at: Option<DateTime<Utc>>,
}

impl SessionState {
    /// Returns the path for the JSON sidecar file corresponding to this
    /// session's transcript.
    ///
    /// Replaces the `.jsonl` extension with `.session.json`.
    #[must_use]
    pub fn sidecar_path(&self) -> Utf8PathBuf {
        sidecar_path_for(&self.transcript_path)
    }

    /// Writes the session state to its JSON sidecar file.
    ///
    /// Parent directories are assumed to exist (they are created when
    /// the transcript file is written).
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Io`] when the file cannot be written.
    pub fn write_sidecar(&self) -> Result<(), IntakeError> {
        let sidecar = self.sidecar_path();
        let parent = sidecar.parent().unwrap_or_else(|| Utf8Path::new("."));
        let file_name = sidecar.file_name().ok_or_else(|| IntakeError::Io {
            message: format!("invalid sidecar path '{sidecar}': no file name"),
        })?;

        let dir = open_dir(parent, "sidecar")?;

        let json = serde_json::to_string_pretty(self).map_err(|error| IntakeError::Io {
            message: format!("failed to serialise session state for '{sidecar}': {error}"),
        })?;

        dir.write(file_name, json).map_err(|error| IntakeError::Io {
            message: format!("failed to write session sidecar '{sidecar}': {error}"),
        })
    }

    /// Reads a session state from a JSON sidecar file.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Io`] when the file cannot be read or
    /// parsed.
    pub fn read_sidecar(path: &Utf8Path) -> Result<Self, IntakeError> {
        let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
        let file_name = path.file_name().ok_or_else(|| IntakeError::Io {
            message: format!("invalid sidecar path '{path}': no file name"),
        })?;

        let dir = open_dir(parent, "sidecar")?;
        let content = dir
            .read_to_string(file_name)
            .map_err(|error| IntakeError::Io {
                message: format!("failed to read session sidecar '{path}': {error}"),
            })?;

        serde_json::from_str(&content).map_err(|error| IntakeError::Io {
            message: format!("failed to parse session sidecar '{path}': {error}"),
        })
    }
}

/// Finds the most recent interrupted session for a given PR.
///
/// Scans all `.session.json` files in `base_dir`, filtering for those
/// matching the specified owner, repository, and PR number with status
/// `Interrupted` and a non-empty `thread_id`. Returns the most recent
/// match by `started_at`, or `None` if no resumable session exists.
///
/// Unparseable sidecar files are silently skipped.
///
/// # Errors
///
/// Returns [`IntakeError::Io`] when the base directory cannot be
/// opened or listed.
pub fn find_interrupted_session(
    base_dir: &Utf8Path,
    owner: &str,
    repository: &str,
    pr_number: u64,
) -> Result<Option<SessionState>, IntakeError> {
    let dir = open_dir(base_dir, "transcript base")?;

    let mut candidates: Vec<SessionState> = Vec::new();

    for entry_result in dir.entries().map_err(|error| IntakeError::Io {
        message: format!("failed to list transcript directory '{base_dir}': {error}"),
    })? {
        let Ok(entry) = entry_result else { continue };

        let Ok(name) = entry.file_name() else {
            continue;
        };
        if !name.ends_with(".session.json") {
            continue;
        }

        let sidecar_path = base_dir.join(&name);
        let Ok(session) = SessionState::read_sidecar(&sidecar_path) else {
            continue;
        };

        if session.owner != owner
            || session.repository != repository
            || session.pr_number != pr_number
        {
            continue;
        }

        if session.status != SessionStatus::Interrupted {
            continue;
        }

        if session.thread_id.is_none() {
            continue;
        }

        candidates.push(session);
    }

    candidates.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(candidates.into_iter().next())
}

/// Derives the sidecar path from a transcript path.
fn sidecar_path_for(transcript_path: &Utf8Path) -> Utf8PathBuf {
    let stem = transcript_path.file_stem().unwrap_or("unknown");
    let parent = transcript_path
        .parent()
        .unwrap_or_else(|| Utf8Path::new("."));
    parent.join(format!("{stem}.session.json"))
}

/// Opens a directory using ambient authority.
fn open_dir(path: &Utf8Path, label: &str) -> Result<Dir, IntakeError> {
    Dir::open_ambient_dir(path, ambient_authority()).map_err(|error| IntakeError::Io {
        message: format!("failed to open {label} directory '{path}': {error}"),
    })
}

#[cfg(test)]
mod tests {
    //! Unit tests for session state persistence and discovery.

    use chrono::TimeZone;
    use rstest::rstest;
    use tempfile::TempDir;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn sample_session(transcript_path: Utf8PathBuf) -> SessionState {
        SessionState {
            status: SessionStatus::Interrupted,
            transcript_path,
            thread_id: Some("thr_abc123".to_owned()),
            owner: "owner".to_owned(),
            repository: "repo".to_owned(),
            pr_number: 42,
            started_at: Utc
                .with_ymd_and_hms(2026, 2, 15, 10, 0, 0)
                .single()
                .expect("test timestamp must be valid"),
            finished_at: Some(
                Utc.with_ymd_and_hms(2026, 2, 15, 10, 5, 0)
                    .single()
                    .expect("test timestamp must be valid"),
            ),
        }
    }

    #[rstest]
    fn session_state_sidecar_path_replaces_extension() {
        let session = sample_session(Utf8PathBuf::from("/tmp/transcripts/run.jsonl"));
        let expected = Utf8PathBuf::from("/tmp/transcripts/run.session.json");
        assert_eq!(session.sidecar_path(), expected);
    }

    #[rstest]
    fn session_state_write_and_read_roundtrip() -> TestResult {
        let temp_dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| "temp directory path must be UTF-8")?;

        let transcript_path = base.join("test-run.jsonl");
        let session = sample_session(transcript_path);

        session.write_sidecar()?;

        let read_back = SessionState::read_sidecar(&session.sidecar_path())?;
        if session != read_back {
            return Err(format!("expected {session:?}, got {read_back:?}").into());
        }

        Ok(())
    }

    #[rstest]
    fn session_state_read_invalid_json_returns_error() -> TestResult {
        let temp_dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| "temp directory path must be UTF-8")?;

        let sidecar = base.join("bad.session.json");
        let dir = Dir::open_ambient_dir(&base, ambient_authority())?;
        dir.write("bad.session.json", "not valid json")?;

        let result = SessionState::read_sidecar(&sidecar);
        if result.is_ok() {
            return Err("expected error for invalid JSON".into());
        }

        Ok(())
    }

    #[rstest]
    fn find_interrupted_session_returns_most_recent() -> TestResult {
        let temp_dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| "temp directory path must be UTF-8")?;

        // Older session
        let mut older = sample_session(base.join("old-run.jsonl"));
        older.started_at = Utc
            .with_ymd_and_hms(2026, 2, 15, 8, 0, 0)
            .single()
            .ok_or("test timestamp must be valid")?;
        older.write_sidecar()?;

        // Newer session
        let mut newer = sample_session(base.join("new-run.jsonl"));
        newer.started_at = Utc
            .with_ymd_and_hms(2026, 2, 15, 12, 0, 0)
            .single()
            .ok_or("test timestamp must be valid")?;
        newer.write_sidecar()?;

        let result = find_interrupted_session(&base, "owner", "repo", 42)?;
        let matched = result.ok_or("expected to find interrupted session")?;
        if matched.started_at != newer.started_at {
            return Err(format!(
                "expected started_at {:?}, got {:?}",
                newer.started_at, matched.started_at
            )
            .into());
        }

        Ok(())
    }

    #[rstest]
    fn find_interrupted_session_ignores_completed() -> TestResult {
        let temp_dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| "temp directory path must be UTF-8")?;

        let mut completed = sample_session(base.join("completed-run.jsonl"));
        completed.status = SessionStatus::Completed;
        completed.write_sidecar()?;

        let found = find_interrupted_session(&base, "owner", "repo", 42)?;
        if found.is_some() {
            return Err("completed sessions should not match".into());
        }

        Ok(())
    }

    #[rstest]
    fn find_interrupted_session_returns_none_when_empty() -> TestResult {
        let temp_dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| "temp directory path must be UTF-8")?;

        let found = find_interrupted_session(&base, "owner", "repo", 42)?;
        if found.is_some() {
            return Err("expected None for empty directory".into());
        }

        Ok(())
    }

    #[rstest]
    fn find_interrupted_session_ignores_sessions_without_thread_id() -> TestResult {
        let temp_dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| "temp directory path must be UTF-8")?;

        let mut no_thread = sample_session(base.join("no-thread.jsonl"));
        no_thread.thread_id = None;
        no_thread.write_sidecar()?;

        let found = find_interrupted_session(&base, "owner", "repo", 42)?;
        if found.is_some() {
            return Err("sessions without thread_id should not match".into());
        }

        Ok(())
    }
}
