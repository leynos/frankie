use super::*;

/// Expected line mapping properties for test assertions.
#[derive(Debug, Clone)]
struct ExpectedLineMapping {
    original: u32,
    current: Option<u32>,
    status: LineMappingStatus,
    offset: Option<i32>,
}

impl ExpectedLineMapping {
    /// Returns expected values for an exact match.
    const fn exact(line: u32) -> Self {
        Self {
            original: line,
            current: Some(line),
            status: LineMappingStatus::Exact,
            offset: Some(0),
        }
    }

    /// Returns expected values for a moved line.
    fn moved(original: u32, current: u32) -> Self {
        let offset = i32::try_from(current).unwrap_or(0) - i32::try_from(original).unwrap_or(0);
        Self {
            original,
            current: Some(current),
            status: LineMappingStatus::Moved,
            offset: Some(offset),
        }
    }

    /// Returns expected values for a deleted line.
    const fn deleted(line: u32) -> Self {
        Self {
            original: line,
            current: None,
            status: LineMappingStatus::Deleted,
            offset: None,
        }
    }

    /// Returns expected values for a line that could not be found.
    const fn not_found(line: u32) -> Self {
        Self {
            original: line,
            current: None,
            status: LineMappingStatus::NotFound,
            offset: None,
        }
    }
}

/// Asserts that a commit snapshot has the expected basic properties.
fn assert_snapshot_has_basic_properties(snapshot: &CommitSnapshot, expected: &CommitMetadata) {
    assert_eq!(snapshot.sha(), &expected.sha);
    let expected_short: String = expected.sha.chars().take(7).collect();
    assert_eq!(snapshot.short_sha(), expected_short);
    assert_eq!(snapshot.message(), &expected.message);
    assert_eq!(snapshot.author(), &expected.author);
    assert_eq!(snapshot.timestamp(), &expected.timestamp);
}

/// Asserts that a line mapping verification has the expected properties.
fn assert_line_mapping(verification: &LineMappingVerification, expected: &ExpectedLineMapping) {
    assert_eq!(verification.original_line(), expected.original);
    assert_eq!(verification.current_line(), expected.current);
    assert_eq!(verification.status(), expected.status);
    assert_eq!(verification.offset(), expected.offset);
}

#[test]
fn commit_snapshot_new() {
    let timestamp = Utc::now();
    let metadata = CommitMetadata::new(
        "abc1234567890".to_owned(),
        "Fix bug in login".to_owned(),
        "Alice".to_owned(),
        timestamp,
    );
    let snapshot = CommitSnapshot::new(metadata.clone());

    assert_snapshot_has_basic_properties(&snapshot, &metadata);
    assert!(snapshot.file_content().is_none());
    assert!(snapshot.file_path().is_none());
}

#[test]
fn commit_snapshot_with_file() {
    let timestamp = Utc::now();
    let metadata = CommitMetadata::new(
        "def5678901234".to_owned(),
        "Add feature".to_owned(),
        "Bob".to_owned(),
        timestamp,
    );
    let snapshot = CommitSnapshot::with_file_content(
        metadata,
        "src/main.rs".to_owned(),
        "fn main() {}".to_owned(),
    );

    assert_eq!(snapshot.file_content(), Some("fn main() {}"));
    assert_eq!(snapshot.file_path(), Some("src/main.rs"));
}

#[test]
fn line_mapping_exact() {
    let verification = LineMappingVerification::exact(42);
    assert_line_mapping(&verification, &ExpectedLineMapping::exact(42));
    assert!(verification.display().contains("exact match"));
}

#[test]
fn line_mapping_moved() {
    let verification = LineMappingVerification::moved(42, 50);
    assert_line_mapping(&verification, &ExpectedLineMapping::moved(42, 50));
    assert!(verification.display().contains("+8"));
}

#[test]
fn line_mapping_moved_up() {
    let verification = LineMappingVerification::moved(50, 42);
    assert_eq!(verification.offset(), Some(-8));
    assert!(verification.display().contains("-8"));
}

#[test]
fn line_mapping_deleted() {
    let verification = LineMappingVerification::deleted(42);
    assert_line_mapping(&verification, &ExpectedLineMapping::deleted(42));
    assert!(verification.display().contains("deleted"));
}

#[test]
fn line_mapping_not_found() {
    let verification = LineMappingVerification::not_found(42);
    assert_line_mapping(&verification, &ExpectedLineMapping::not_found(42));
    assert!(verification.display().contains("not found"));
}

#[test]
fn status_symbols() {
    assert_eq!(LineMappingStatus::Exact.symbol(), "✓");
    assert_eq!(LineMappingStatus::Moved.symbol(), "→");
    assert_eq!(LineMappingStatus::Deleted.symbol(), "✗");
    assert_eq!(LineMappingStatus::NotFound.symbol(), "?");
}
