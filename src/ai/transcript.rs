//! Transcript path resolution and JSONL transcript persistence.
//!
//! Codex execution emits JSON Lines (JSONL) events. This module standardizes
//! where transcripts are written and provides a small writer abstraction for
//! appending event lines.

use std::io::Write;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use chrono::{DateTime, Utc};

use crate::github::IntakeError;

/// Metadata used to derive transcript file names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptMetadata {
    owner: String,
    repository: String,
    pr_number: u64,
}

impl TranscriptMetadata {
    /// Creates transcript metadata for a pull request.
    #[must_use]
    pub fn new(owner: impl Into<String>, repository: impl Into<String>, pr_number: u64) -> Self {
        Self {
            owner: owner.into(),
            repository: repository.into(),
            pr_number,
        }
    }
}

/// Writer for transcript JSONL lines.
#[derive(Debug)]
pub struct TranscriptWriter {
    path: Utf8PathBuf,
    file: cap_std::fs_utf8::File,
}

impl TranscriptWriter {
    /// Creates a transcript file and returns a line-oriented writer.
    ///
    /// Parent directories are created when needed.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Io`] when the parent directory cannot be created
    /// or the transcript file cannot be created.
    pub fn create(path: &Utf8Path) -> Result<Self, IntakeError> {
        let (resolved, file) = create_file_with_parents(path, "transcript")?;
        Ok(Self {
            path: resolved,
            file,
        })
    }

    /// Appends one transcript line.
    ///
    /// A trailing newline is always written.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Io`] when the write operation fails.
    pub fn append_line(&mut self, line: &str) -> Result<(), IntakeError> {
        writeln!(self.file, "{line}").map_err(|error| IntakeError::Io {
            message: format!("failed to append transcript line '{}': {error}", self.path),
        })
    }

    /// Flushes buffered transcript output.
    ///
    /// # Errors
    ///
    /// Returns [`IntakeError::Io`] when flushing fails.
    pub fn flush(&mut self) -> Result<(), IntakeError> {
        self.file.flush().map_err(|error| IntakeError::Io {
            message: format!("failed to flush transcript '{}': {error}", self.path),
        })
    }

    /// Returns the transcript path.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        self.path.as_path()
    }
}

/// Resolves the default transcript root directory.
///
/// Defaults to `${XDG_STATE_HOME}/frankie/codex-transcripts` when
/// `XDG_STATE_HOME` is set, else to
/// `${HOME}/.local/state/frankie/codex-transcripts`.
///
/// # Errors
///
/// Returns [`IntakeError::Configuration`] when neither `XDG_STATE_HOME` nor
/// `HOME` is available.
pub fn default_transcript_base_dir() -> Result<Utf8PathBuf, IntakeError> {
    let xdg = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|v| !v.is_empty());
    let home = std::env::var("HOME").ok().filter(|v| !v.is_empty());

    resolve_transcript_base_dir(xdg.as_deref(), home.as_deref())
}

/// Resolves the transcript root from optional environment values.
///
/// This helper exists to keep environment-sensitive logic unit-testable
/// without mutating process environment variables in tests.
pub(crate) fn resolve_transcript_base_dir(
    xdg_state_home: Option<&str>,
    home: Option<&str>,
) -> Result<Utf8PathBuf, IntakeError> {
    if let Some(state_home) = xdg_state_home {
        return Ok(Utf8PathBuf::from(state_home)
            .join("frankie")
            .join("codex-transcripts"));
    }

    if let Some(home_dir) = home {
        return Ok(Utf8PathBuf::from(home_dir)
            .join(".local")
            .join("state")
            .join("frankie")
            .join("codex-transcripts"));
    }

    Err(IntakeError::Configuration {
        message: "unable to resolve transcript directory: \
                  neither XDG_STATE_HOME nor HOME is set"
            .to_owned(),
    })
}

/// Builds a deterministic transcript file path.
#[must_use]
pub fn transcript_path(
    base_dir: &Utf8Path,
    metadata: &TranscriptMetadata,
    now: DateTime<Utc>,
) -> Utf8PathBuf {
    let timestamp = now.format("%Y%m%dT%H%M%SZ");
    let owner = sanitize_segment(&metadata.owner);
    let repository = sanitize_segment(&metadata.repository);
    let file_name = format!(
        "{owner}-{repository}-pr-{}-{timestamp}.jsonl",
        metadata.pr_number
    );

    base_dir.join(file_name)
}

fn sanitize_segment(segment: &str) -> String {
    const fn is_safe_for_filename(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
    }

    segment
        .chars()
        .map(|ch| if is_safe_for_filename(ch) { ch } else { '-' })
        .collect()
}

/// Creates a file at `path`, ensuring parent directories exist first.
///
/// Returns the canonical path and the opened file handle.  All directory
/// and path logic is centralised here so callers do not need to coordinate
/// `ensure_parent_dirs` and `open_dir_for_path` separately.
fn create_file_with_parents(
    path: &Utf8Path,
    path_type: &str,
) -> Result<(Utf8PathBuf, cap_std::fs_utf8::File), IntakeError> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path.file_name().ok_or_else(|| IntakeError::Io {
        message: format!("invalid {path_type} path '{path}': no file name"),
    })?;

    let (dir, rel_parent) = if parent == Utf8Path::new(".") || parent.as_str().is_empty() {
        (
            Dir::open_ambient_dir(".", ambient_authority()).map_err(|error| IntakeError::Io {
                message: format!("failed to open current directory for {path_type}s: {error}"),
            })?,
            Utf8Path::new("."),
        )
    } else if parent.is_absolute() {
        let root =
            Dir::open_ambient_dir("/", ambient_authority()).map_err(|error| IntakeError::Io {
                message: format!("failed to open root directory for {path_type}s: {error}"),
            })?;
        let rel = parent.strip_prefix("/").map_err(|_| IntakeError::Io {
            message: format!("failed to normalise {path_type} directory '{parent}'"),
        })?;
        (root, rel)
    } else {
        (
            Dir::open_ambient_dir(".", ambient_authority()).map_err(|error| IntakeError::Io {
                message: format!("failed to open current directory for {path_type}s: {error}"),
            })?,
            parent,
        )
    };

    let target_dir = if !rel_parent.as_str().is_empty() && rel_parent != Utf8Path::new(".") {
        dir.create_dir_all(rel_parent)
            .map_err(|error| IntakeError::Io {
                message: format!("failed to create {path_type} directory '{parent}': {error}"),
            })?;
        dir.open_dir(rel_parent).map_err(|error| IntakeError::Io {
            message: format!("failed to open {path_type} directory '{parent}': {error}"),
        })?
    } else {
        dir
    };

    let file = target_dir
        .create(file_name)
        .map_err(|error| IntakeError::Io {
            message: format!("failed to create {path_type} file '{path}': {error}"),
        })?;

    Ok((path.to_path_buf(), file))
}

#[cfg(test)]
mod tests {
    //! Unit tests for transcript path resolution and JSONL persistence.

    use chrono::TimeZone;
    use rstest::rstest;
    use tempfile::TempDir;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[rstest]
    fn resolve_transcript_base_dir_prefers_xdg_state_home() -> TestResult {
        let path = resolve_transcript_base_dir(Some("/tmp/state-root"), Some("/home/example"))?;

        let expected = Utf8PathBuf::from("/tmp/state-root/frankie/codex-transcripts");
        if path != expected {
            return Err(format!("expected {expected:?}, got {path:?}").into());
        }

        Ok(())
    }

    #[rstest]
    fn resolve_transcript_base_dir_falls_back_to_home() -> TestResult {
        let path = resolve_transcript_base_dir(None, Some("/home/example"))?;

        let expected = Utf8PathBuf::from("/home/example/.local/state/frankie/codex-transcripts");
        if path != expected {
            return Err(format!("expected {expected:?}, got {path:?}").into());
        }

        Ok(())
    }

    #[rstest]
    fn resolve_transcript_base_dir_errors_without_any_base() -> TestResult {
        let result = resolve_transcript_base_dir(None, None);
        if !matches!(result, Err(IntakeError::Configuration { .. })) {
            return Err(format!("expected Configuration error, got {result:?}").into());
        }

        Ok(())
    }

    #[rstest]
    fn transcript_path_is_deterministic_and_sanitized() -> TestResult {
        let metadata = TranscriptMetadata::new("owner/name", "repo.name", 42);
        let now = Utc
            .with_ymd_and_hms(2026, 2, 12, 10, 11, 12)
            .single()
            .ok_or("fixed timestamp should be valid")?;

        let path = transcript_path(Utf8Path::new("/tmp/transcripts"), &metadata, now);

        let expected =
            Utf8PathBuf::from("/tmp/transcripts/owner-name-repo-name-pr-42-20260212T101112Z.jsonl");
        if path != expected {
            return Err(format!("expected {expected:?}, got {path:?}").into());
        }

        Ok(())
    }

    #[rstest]
    fn transcript_writer_creates_parent_dirs_and_appends_lines() -> TestResult {
        let temp_dir = TempDir::new()?;
        let base = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .map_err(|_| "temp directory path must be UTF-8")?;
        let path = base.join("nested").join("run.jsonl");

        let mut writer = TranscriptWriter::create(&path)?;
        writer.append_line("line-1")?;
        writer.append_line("line-2")?;
        writer.flush()?;

        let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
        let dir = Dir::open_ambient_dir(parent, ambient_authority())?;
        let file_name = path.file_name().ok_or("path has no file name")?;
        let content = dir.read_to_string(file_name)?;
        if content != "line-1\nline-2\n" {
            return Err(format!("expected 'line-1\\nline-2\\n', got {content:?}").into());
        }

        Ok(())
    }
}
