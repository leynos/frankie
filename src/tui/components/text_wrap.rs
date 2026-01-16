//! Text wrapping utilities for terminal display.
//!
//! Provides two wrapping strategies:
//! - **Character-based wrapping** (`wrap_to_width`): Hard-wraps at exactly N
//!   characters, used for code blocks where preserving exact layout matters.
//! - **Word-based wrapping** (`wrap_text`): Wraps at word boundaries while
//!   preserving indentation and multiple spaces, used for prose text.

/// Wraps a single line to a maximum width using Unicode scalar value count.
///
/// Counts and wraps by Unicode scalar values (code points), not bytes. This
/// means each `char` counts as 1 regardless of its UTF-8 byte length.
///
/// **Note**: This function does not account for terminal display width. CJK
/// characters and many emoji occupy 2 terminal columns but count as 1 here.
/// Combined grapheme clusters (e.g., 'e' + combining accent, or emoji with
/// skin tone modifiers) also count each code point separately. For accurate
/// display-width wrapping, consider crates like `unicode-width` or
/// `unicode-segmentation`.
///
/// # Arguments
///
/// * `line` - The line to wrap
/// * `max_width` - Maximum Unicode scalar values (code points) per line
#[must_use]
pub fn wrap_to_width(line: &str, max_width: usize) -> String {
    if max_width == 0 {
        return line.to_owned();
    }

    let char_count = line.chars().count();
    if char_count <= max_width {
        return line.to_owned();
    }

    // Estimate extra capacity for newlines: one per max_width chars
    let extra_newlines = char_count
        .saturating_sub(1)
        .checked_div(max_width)
        .unwrap_or(0);
    let mut result = String::with_capacity(line.len() + extra_newlines);
    let mut current_width = 0;

    for ch in line.chars() {
        if current_width >= max_width {
            result.push('\n');
            current_width = 0;
        }
        result.push(ch);
        current_width += 1;
    }

    result
}

/// Wraps all lines of a multi-line text block to a maximum width.
///
/// Applies [`wrap_to_width`] to each line and joins the results.
/// This is the primary entry point for wrapping code blocks.
///
/// # Arguments
///
/// * `text` - The multi-line text
/// * `max_width` - Maximum characters per line
#[must_use]
pub fn wrap_code_block(text: &str, max_width: usize) -> String {
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };

    lines.fold(wrap_to_width(first, max_width), |mut acc, line| {
        acc.push('\n');
        acc.push_str(&wrap_to_width(line, max_width));
        acc
    })
}

/// Wraps text to a maximum width, preserving existing whitespace.
///
/// Unlike simple word-wrap, this preserves:
/// - Leading indentation on each line
/// - Multiple spaces between words
/// - Empty lines (paragraph breaks)
///
/// Lines are only wrapped when they exceed `max_width`. Wrapped
/// continuation lines preserve the original line's indentation.
///
/// # Arguments
///
/// * `text` - The text to wrap
/// * `max_width` - Maximum characters per line
#[must_use]
pub fn wrap_text(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return text.to_owned();
    }

    text.lines()
        .map(|line| wrap_line_preserving_indent(line, max_width))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wraps a single line while preserving its leading indentation.
///
/// If the line fits within `max_width`, returns it unchanged.
/// Otherwise, wraps at word boundaries, using the original
/// indentation for continuation lines.
fn wrap_line_preserving_indent(line: &str, max_width: usize) -> String {
    let line_width = line.chars().count();

    // Short lines pass through unchanged
    if line_width <= max_width {
        return line.to_owned();
    }

    // Extract leading whitespace (inlined logic)
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    let (indent, content) = line.split_at(indent_len);
    let indent_width = indent.chars().count();

    // If indent alone exceeds max_width, just hard-wrap
    if indent_width >= max_width {
        return wrap_to_width(line, max_width);
    }

    let available_width = max_width.saturating_sub(indent_width);
    wrap_content_with_indent(content, indent, available_width)
}

/// Context for wrapping content with indentation.
struct WrapContext<'a> {
    indent: &'a str,
    available_width: usize,
    lines: Vec<String>,
    current_line: String,
    content_width: usize,
}

impl<'a> WrapContext<'a> {
    fn new(indent: &'a str, available_width: usize) -> Self {
        Self {
            indent,
            available_width,
            lines: Vec::new(),
            current_line: String::from(indent),
            content_width: 0,
        }
    }

    const fn is_line_empty(&self) -> bool {
        self.content_width == 0
    }

    fn start_new_line(&mut self) {
        let old_line = std::mem::replace(&mut self.current_line, String::from(self.indent));
        self.lines.push(old_line);
        self.content_width = 0;
    }

    fn process_word(&mut self, word: &str) {
        let word_len = word.chars().count();

        // Check if we need to wrap before this word
        if !self.is_line_empty() && self.content_width + word_len > self.available_width {
            self.start_new_line();
        }

        // Handle words longer than available width (hard wrap)
        if word_len > self.available_width && self.is_line_empty() {
            self.process_long_word(word);
        } else {
            self.current_line.push_str(word);
            self.content_width += word_len;
        }
    }

    fn process_long_word(&mut self, word: &str) {
        let wrapped = wrap_to_width(word, self.available_width);
        let parts: Vec<&str> = wrapped.lines().collect();
        let last_idx = parts.len().saturating_sub(1);

        for (i, part) in parts.into_iter().enumerate() {
            self.current_line.push_str(part);
            self.content_width += part.chars().count();
            if i < last_idx {
                self.start_new_line();
            }
        }
    }

    fn process_space(&mut self, space: &str) {
        if self.is_line_empty() {
            return;
        }

        let space_len = space.chars().count();
        if self.content_width + space_len <= self.available_width {
            self.current_line.push_str(space);
            self.content_width += space_len;
        } else {
            self.start_new_line();
        }
    }

    fn finish(mut self) -> String {
        self.lines.push(self.current_line);
        self.lines.join("\n")
    }
}

/// Wraps content with a given indent prefix.
///
/// Words are wrapped to fit within `available_width`, and each
/// wrapped line is prefixed with `indent`.
fn wrap_content_with_indent(content: &str, indent: &str, available_width: usize) -> String {
    let mut ctx = WrapContext::new(indent, available_width);

    for segment in split_preserving_spaces(content) {
        match segment {
            Segment::Word(word) => ctx.process_word(&word),
            Segment::Space(space) => ctx.process_space(&space),
        }
    }

    ctx.finish()
}

/// A segment of text: either a word or whitespace.
enum Segment {
    Word(String),
    Space(String),
}

/// Splits content into alternating word and space segments.
///
/// Preserves the exact spacing between words, allowing the wrapper
/// to maintain multiple spaces where they appear in the original.
fn split_preserving_spaces(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_whitespace: Option<bool> = None;

    for ch in content.chars() {
        let is_space = ch.is_whitespace();

        match in_whitespace {
            None => {
                // First character
                current.push(ch);
                in_whitespace = Some(is_space);
            }
            Some(was_space) if was_space == is_space => {
                // Same type, continue accumulating
                current.push(ch);
            }
            Some(was_space) => {
                // Type changed, push current segment and start new one
                segments.push(create_segment(std::mem::take(&mut current), was_space));
                current.push(ch);
                in_whitespace = Some(is_space);
            }
        }
    }

    // Push final segment if non-empty
    finalize_segment(&mut segments, current, in_whitespace);

    segments
}

/// Creates a segment based on whether it contains whitespace.
const fn create_segment(content: String, is_space: bool) -> Segment {
    if is_space {
        Segment::Space(content)
    } else {
        Segment::Word(content)
    }
}

/// Finalises and pushes the last segment if non-empty.
fn finalize_segment(segments: &mut Vec<Segment>, current: String, in_whitespace: Option<bool>) {
    if let Some(is_space) = in_whitespace
        && !current.is_empty()
    {
        segments.push(create_segment(current, is_space));
    }
}

#[cfg(test)]
#[path = "text_wrap_tests.rs"]
mod tests;
