//! Stable ordering for exported comments.
//!
//! Comments are sorted by file path (alphabetical), then line number
//! (ascending), then comment ID (ascending). Comments with missing file paths
//! or line numbers are sorted last within their group.

use std::cmp::Ordering;

use super::model::ExportedComment;

/// Sorts comments in stable order for deterministic export.
///
/// The ordering is:
/// 1. File path (alphabetical, `None` values last)
/// 2. Line number (ascending, `None` values last)
/// 3. Comment ID (ascending)
///
/// # Examples
///
/// ```
/// use frankie::ReviewComment;
/// # use std::path::Path;
///
/// // Comments are sorted by file, then line, then ID
/// ```
pub fn sort_comments(comments: &mut [ExportedComment]) {
    comments.sort_by(compare_comments);
}

/// Compares two comments for stable ordering.
fn compare_comments(a: &ExportedComment, b: &ExportedComment) -> Ordering {
    // First, compare by file path (None values sort last)
    let file_cmp = compare_options(a.file_path.as_ref(), b.file_path.as_ref());
    if file_cmp != Ordering::Equal {
        return file_cmp;
    }

    // Then, compare by line number (None values sort last)
    let line_cmp = compare_options(a.line_number.as_ref(), b.line_number.as_ref());
    if line_cmp != Ordering::Equal {
        return line_cmp;
    }

    // Finally, compare by ID (always present)
    a.id.cmp(&b.id)
}

/// Compares two optional values, sorting `None` after `Some`.
fn compare_options<T: Ord>(a: Option<&T>, b: Option<&T>) -> Ordering {
    match (a, b) {
        (Some(a_val), Some(b_val)) => a_val.cmp(b_val),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
#[expect(clippy::indexing_slicing, reason = "test assertions use known indices")]
mod tests {
    use rstest::rstest;

    use super::*;

    fn make_comment(id: u64, file_path: Option<&str>, line_number: Option<u32>) -> ExportedComment {
        ExportedComment {
            id,
            file_path: file_path.map(String::from),
            line_number,
            author: None,
            original_line_number: None,
            body: None,
            diff_hunk: None,
            commit_sha: None,
            in_reply_to_id: None,
            created_at: None,
        }
    }

    fn assert_sorted_order(mut comments: Vec<ExportedComment>, expected_ids: &[u64]) {
        sort_comments(&mut comments);
        let actual_ids: Vec<u64> = comments.iter().map(|c| c.id).collect();
        assert_eq!(actual_ids, expected_ids);
    }

    #[rstest]
    #[case::sorts_by_file_path_first(
        vec![
            make_comment(1, Some("src/z.rs"), Some(10)),
            make_comment(2, Some("src/a.rs"), Some(10)),
            make_comment(3, Some("src/m.rs"), Some(10)),
        ],
        &[2, 3, 1],
        "file paths should sort alphabetically"
    )]
    #[case::sorts_by_line_number_within_file(
        vec![
            make_comment(1, Some("src/lib.rs"), Some(100)),
            make_comment(2, Some("src/lib.rs"), Some(10)),
            make_comment(3, Some("src/lib.rs"), Some(50)),
        ],
        &[2, 3, 1],
        "line numbers should sort ascending within same file"
    )]
    #[case::sorts_by_id_when_file_and_line_match(
        vec![
            make_comment(300, Some("src/lib.rs"), Some(42)),
            make_comment(100, Some("src/lib.rs"), Some(42)),
            make_comment(200, Some("src/lib.rs"), Some(42)),
        ],
        &[100, 200, 300],
        "IDs should sort ascending when file and line match"
    )]
    #[case::none_file_paths_sort_last(
        vec![
            make_comment(1, None, Some(10)),
            make_comment(2, Some("src/lib.rs"), Some(10)),
            make_comment(3, None, Some(5)),
        ],
        &[2, 3, 1],
        "None file paths should sort after Some file paths"
    )]
    fn sorting_behaviour(
        #[case] comments: Vec<ExportedComment>,
        #[case] expected_ids: &[u64],
        #[case] description: &str,
    ) {
        let _ = description; // Used for test case naming only
        assert_sorted_order(comments, expected_ids);
    }

    #[rstest]
    fn none_line_numbers_sort_last_within_file() {
        let mut comments = vec![
            make_comment(1, Some("src/lib.rs"), None),
            make_comment(2, Some("src/lib.rs"), Some(10)),
            make_comment(3, Some("src/lib.rs"), None),
        ];

        sort_comments(&mut comments);

        assert_eq!(comments[0].line_number, Some(10));
        assert!(comments[1].line_number.is_none());
        assert!(comments[2].line_number.is_none());
        // Within None group, sorted by ID
        assert_eq!(comments[1].id, 1);
        assert_eq!(comments[2].id, 3);
    }

    #[rstest]
    fn empty_list_is_no_op() {
        let mut comments: Vec<ExportedComment> = vec![];
        sort_comments(&mut comments);
        assert!(comments.is_empty());
    }

    #[rstest]
    fn single_element_unchanged() {
        let mut comments = vec![make_comment(42, Some("test.rs"), Some(1))];
        sort_comments(&mut comments);
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, 42);
    }

    #[rstest]
    fn complex_ordering_scenario() {
        let mut comments = vec![
            make_comment(5, Some("src/b.rs"), Some(20)),
            make_comment(1, None, None),
            make_comment(3, Some("src/a.rs"), Some(30)),
            make_comment(2, Some("src/a.rs"), Some(10)),
            make_comment(4, Some("src/b.rs"), Some(10)),
            make_comment(6, Some("src/a.rs"), None),
        ];

        sort_comments(&mut comments);

        // Expected order:
        // src/a.rs:10 (id=2), src/a.rs:30 (id=3), src/a.rs:None (id=6)
        // src/b.rs:10 (id=4), src/b.rs:20 (id=5)
        // None:None (id=1)
        assert_eq!(comments[0].id, 2); // src/a.rs:10
        assert_eq!(comments[1].id, 3); // src/a.rs:30
        assert_eq!(comments[2].id, 6); // src/a.rs:None
        assert_eq!(comments[3].id, 4); // src/b.rs:10
        assert_eq!(comments[4].id, 5); // src/b.rs:20
        assert_eq!(comments[5].id, 1); // None:None
    }
}
