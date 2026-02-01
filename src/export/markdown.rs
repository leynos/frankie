//! Markdown formatter for exported comments.
//!
//! Generates human-readable Markdown output with syntax-highlighted code
//! blocks for diff context.

use std::io::Write;

use camino::Utf8Path;

use crate::github::IntakeError;

use super::model::ExportedComment;

/// Writes comments in Markdown format to the given writer.
///
/// The output includes a header with the PR URL, followed by each comment
/// with its file location, author, timestamp, body text, and code context
/// in a fenced code block.
///
/// # Errors
///
/// Returns [`IntakeError::Io`] if writing to the output fails.
pub fn write_markdown<W: Write>(
    writer: &mut W,
    comments: &[ExportedComment],
    pr_url: &str,
) -> Result<(), IntakeError> {
    write_header(writer, pr_url)?;

    for comment in comments {
        write_comment_section(writer, comment)?;
    }

    Ok(())
}

/// Writes the Markdown header with PR URL.
fn write_header<W: Write>(writer: &mut W, pr_url: &str) -> Result<(), IntakeError> {
    writeln!(writer, "# Review Comments Export").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;
    writeln!(writer, "PR: {pr_url}").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;
    Ok(())
}

/// Writes a single comment section.
fn write_comment_section<W: Write>(
    writer: &mut W,
    comment: &ExportedComment,
) -> Result<(), IntakeError> {
    writeln!(writer, "---").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;

    // File location heading
    write_location_heading(writer, comment)?;

    // Metadata (author and timestamp)
    write_metadata(writer, comment)?;

    // Comment body
    if let Some(body) = &comment.body {
        writeln!(writer).map_err(|e| io_error(&e))?;
        writeln!(writer, "{body}").map_err(|e| io_error(&e))?;
    }

    // Code context
    if let Some(diff_hunk) = &comment.diff_hunk {
        write_code_block(writer, comment.file_path.as_deref(), diff_hunk)?;
    }

    writeln!(writer).map_err(|e| io_error(&e))?;
    Ok(())
}

/// Writes the file location heading.
fn write_location_heading<W: Write>(
    writer: &mut W,
    comment: &ExportedComment,
) -> Result<(), IntakeError> {
    let location = match (&comment.file_path, comment.line_number) {
        (Some(path), Some(line)) => format!("{path}:{line}"),
        (Some(path), None) => path.clone(),
        (None, Some(line)) => format!("(unknown file):{line}"),
        (None, None) => "(unknown location)".to_owned(),
    };
    writeln!(writer, "## {location}").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;
    Ok(())
}

/// Writes comment metadata (author and timestamp).
fn write_metadata<W: Write>(writer: &mut W, comment: &ExportedComment) -> Result<(), IntakeError> {
    if let Some(author) = &comment.author {
        writeln!(writer, "**Reviewer:** {author}").map_err(|e| io_error(&e))?;
    }
    if let Some(created_at) = &comment.created_at {
        writeln!(writer, "**Created:** {created_at}").map_err(|e| io_error(&e))?;
    }
    Ok(())
}

/// Writes a fenced code block with language hint from file extension.
///
/// Uses a fence length that exceeds any backtick runs in the diff hunk to
/// prevent nested code fences from breaking the Markdown output.
fn write_code_block<W: Write>(
    writer: &mut W,
    file_path: Option<&str>,
    diff_hunk: &str,
) -> Result<(), IntakeError> {
    let language = file_path
        .and_then(|p| Utf8Path::new(p).extension())
        .map_or("diff", extension_to_language);

    let fence = compute_fence(diff_hunk);
    writeln!(writer).map_err(|e| io_error(&e))?;
    writeln!(writer, "{fence}{language}").map_err(|e| io_error(&e))?;
    writeln!(writer, "{diff_hunk}").map_err(|e| io_error(&e))?;
    writeln!(writer, "{fence}").map_err(|e| io_error(&e))?;
    Ok(())
}

/// Computes a fence string that exceeds any backtick run in the content.
fn compute_fence(content: &str) -> String {
    let max_backticks = content.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence_len = max_backticks.max(2) + 1;
    "`".repeat(fence_len)
}

/// Extension-to-language mapping entries.
const EXTENSION_MAPPINGS: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("js", "javascript"),
    ("ts", "typescript"),
    ("jsx", "jsx"),
    ("tsx", "tsx"),
    ("rb", "ruby"),
    ("go", "go"),
    ("java", "java"),
    ("kt", "kotlin"),
    ("kts", "kotlin"),
    ("swift", "swift"),
    ("c", "c"),
    ("cpp", "cpp"),
    ("cc", "cpp"),
    ("cxx", "cpp"),
    ("h", "cpp"),
    ("hpp", "cpp"),
    ("cs", "csharp"),
    ("php", "php"),
    ("sh", "bash"),
    ("bash", "bash"),
    ("zsh", "zsh"),
    ("fish", "fish"),
    ("ps1", "powershell"),
    ("sql", "sql"),
    ("md", "markdown"),
    ("json", "json"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("toml", "toml"),
    ("xml", "xml"),
    ("html", "html"),
    ("htm", "html"),
    ("css", "css"),
    ("scss", "scss"),
    ("sass", "scss"),
    ("less", "less"),
];

/// Maps file extensions to Markdown code block language hints.
fn extension_to_language(ext: &str) -> &'static str {
    let ext_lower = ext.to_lowercase();
    EXTENSION_MAPPINGS
        .iter()
        .find(|(e, _)| *e == ext_lower)
        .map_or("diff", |(_, lang)| lang)
}

/// Converts an I/O error to an [`IntakeError::Io`].
fn io_error(error: &std::io::Error) -> IntakeError {
    IntakeError::Io {
        message: error.to_string(),
    }
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
