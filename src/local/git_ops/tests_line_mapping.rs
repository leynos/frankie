//! Line-mapping verification tests for Git operations.
//!
//! Covers exact matches, offset shifts within hunks, deletions, and
//! complex multi-hunk scenarios against real temporary repositories.

use rstest::rstest;

use super::*;

#[rstest]
fn test_line_mapping_exact(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")])?;

    let ops = Git2Operations::from_repository(repo);
    let request =
        LineMappingRequest::new(oid.to_string(), oid.to_string(), "test.txt".to_owned(), 2);
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Exact);
    assert_eq!(verification.original_line(), 2);
    assert_eq!(verification.current_line(), Some(2));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_no_change(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let oid1 = create_commit(&repo, "Add file", &[("test.txt", "line1\nline2\nline3")])?;
    let oid2 = create_commit(&repo, "Other file", &[("other.txt", "other content")])?;

    let ops = Git2Operations::from_repository(repo);
    let request =
        LineMappingRequest::new(oid1.to_string(), oid2.to_string(), "test.txt".to_owned(), 2);
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Exact);

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_for_unchanged_context_line_in_complex_hunk(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;

    // Original: a\nb\nc\nd\n
    let old_oid = create_commit(
        &repo,
        "Add complex hunk base",
        &[("test.txt", "a\nb\nc\nd\n")],
    )?;

    // New: a\nX\nb\nY\nd\n
    // b (old line 2) is an unchanged context line that shifts to new line 3.
    // c (old line 3) is deleted.
    // d (old line 4) is an unchanged context line that shifts to new line 5.
    let new_oid = create_commit(
        &repo,
        "Apply interleaved insertions and deletions",
        &[("test.txt", "a\nX\nb\nY\nd\n")],
    )?;

    let ops = Git2Operations::from_repository(repo);

    // Map old line 2 (b) — should shift to new line 3 (offset +1).
    let request_b = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        2,
    );
    let verify_b = ops.verify_line_mapping(&request_b)?;

    assert_eq!(verify_b.status(), LineMappingStatus::Moved);
    assert_eq!(verify_b.original_line(), 2);
    assert_eq!(verify_b.current_line(), Some(3));
    assert_eq!(verify_b.offset(), Some(1));

    // Map old line 4 (d) — should shift to new line 5 (offset +1).
    let request_d = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        4,
    );
    let verify_d = ops.verify_line_mapping(&request_d)?;

    assert_eq!(verify_d.status(), LineMappingStatus::Moved);
    assert_eq!(verify_d.original_line(), 4);
    assert_eq!(verify_d.current_line(), Some(5));
    assert_eq!(verify_d.offset(), Some(1));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_shifts_when_line_moves_within_hunk(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let old_oid = create_commit(
        &repo,
        "Add initial file",
        &[("test.txt", "alpha\nbeta\ngamma\n")],
    )?;
    let new_oid = create_commit(
        &repo,
        "Insert line at top",
        &[("test.txt", "inserted\nalpha\nbeta\ngamma\n")],
    )?;

    let ops = Git2Operations::from_repository(repo);
    let request = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        2,
    );
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Moved);
    assert_eq!(verification.original_line(), 2);
    assert_eq!(verification.current_line(), Some(3));
    assert_eq!(verification.offset(), Some(1));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_shifts_after_deletion_within_hunk(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let old_oid = create_commit(
        &repo,
        "Add initial file",
        &[("test.txt", "drop\nkeep-one\nkeep-two\n")],
    )?;
    let new_oid = create_commit(
        &repo,
        "Delete first line",
        &[("test.txt", "keep-one\nkeep-two\n")],
    )?;

    let ops = Git2Operations::from_repository(repo);
    let request = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        3,
    );
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Moved);
    assert_eq!(verification.original_line(), 3);
    assert_eq!(verification.current_line(), Some(2));
    assert_eq!(verification.offset(), Some(-1));

    drop(dir);
    Ok(())
}

#[rstest]
fn test_line_mapping_marks_deleted_line_inside_hunk(
    test_repo: Result<(TempDir, Repository), TestError>,
) -> Result<(), TestError> {
    let (dir, repo) = test_repo?;
    let old_oid = create_commit(
        &repo,
        "Add initial file",
        &[("test.txt", "line-1\nline-2\nline-3\n")],
    )?;
    let new_oid = create_commit(
        &repo,
        "Delete middle line",
        &[("test.txt", "line-1\nline-3\n")],
    )?;

    let ops = Git2Operations::from_repository(repo);
    let request = LineMappingRequest::new(
        old_oid.to_string(),
        new_oid.to_string(),
        "test.txt".to_owned(),
        2,
    );
    let verification = ops.verify_line_mapping(&request)?;

    assert_eq!(verification.status(), LineMappingStatus::Deleted);
    assert_eq!(verification.original_line(), 2);
    assert_eq!(verification.current_line(), None);
    assert_eq!(verification.offset(), None);

    drop(dir);
    Ok(())
}
