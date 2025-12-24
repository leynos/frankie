//! Output formatting utilities for CLI operations.

use std::io::{self, Write};

use frankie::{IntakeError, PaginatedPullRequests, PullRequestDetails};

/// Writes a summary of pull request details to stdout.
pub fn write_pr_summary(details: &PullRequestDetails) -> Result<(), IntakeError> {
    let mut stdout = io::stdout().lock();
    write_pr_summary_to(&mut stdout, details)
}

/// Writes a summary of pull request details to the given writer.
pub fn write_pr_summary_to<W: Write>(
    writer: &mut W,
    details: &PullRequestDetails,
) -> Result<(), IntakeError> {
    let title = details
        .metadata
        .title
        .as_deref()
        .unwrap_or("untitled pull request");
    let author = details
        .metadata
        .author
        .as_deref()
        .unwrap_or("unknown author");
    let url = details
        .metadata
        .html_url
        .as_deref()
        .unwrap_or("no HTML URL provided");
    let message = format!(
        "Loaded PR #{} by {author}: {title}\nURL: {url}\nComments: {}",
        details.metadata.number,
        details.comments.len()
    );

    writeln!(writer, "{message}").map_err(|error| IntakeError::Io {
        message: error.to_string(),
    })
}

/// Writes a summary of paginated pull requests to the given writer.
pub fn write_listing_summary<W: Write>(
    writer: &mut W,
    result: &PaginatedPullRequests,
    owner: &str,
    repo: &str,
) -> Result<(), IntakeError> {
    let page_info = &result.page_info;

    writeln!(writer, "Pull requests for {owner}/{repo}:").map_err(|e| io_error(&e))?;
    writeln!(writer).map_err(|e| io_error(&e))?;

    for pr in &result.items {
        let title = pr.title.as_deref().unwrap_or("(no title)");
        let author = pr.author.as_deref().unwrap_or("unknown");
        let state = pr.state.as_deref().unwrap_or("unknown");
        writeln!(writer, "  #{} [{state}] {title} (@{author})", pr.number)
            .map_err(|e| io_error(&e))?;
    }

    writeln!(writer).map_err(|e| io_error(&e))?;
    writeln!(
        writer,
        "Page {} of {} ({} PRs shown)",
        page_info.current_page(),
        page_info.total_pages().unwrap_or(1),
        result.items.len()
    )
    .map_err(|e| io_error(&e))?;

    if page_info.has_next() {
        writeln!(writer, "More pages available.").map_err(|e| io_error(&e))?;
    }

    Ok(())
}

/// Converts an I/O error to an [`IntakeError::Io`].
pub(crate) fn io_error(error: &io::Error) -> IntakeError {
    IntakeError::Io {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use frankie::github::{PageInfo, PullRequestMetadata};
    use frankie::{PaginatedPullRequests, PullRequestDetails, PullRequestSummary, RateLimitInfo};

    use super::{write_listing_summary, write_pr_summary_to};

    #[test]
    fn write_listing_summary_includes_items_and_pagination() {
        let page_info = PageInfo::builder(2, 50)
            .total_pages(Some(3))
            .has_next(true)
            .has_prev(true)
            .build();
        let result = PaginatedPullRequests {
            items: vec![PullRequestSummary {
                number: 42,
                title: Some("Add pagination".to_owned()),
                state: Some("open".to_owned()),
                author: Some("octocat".to_owned()),
                created_at: None,
                updated_at: None,
            }],
            page_info,
            rate_limit: Some(RateLimitInfo::new(5000, 4999, 1_700_000_000)),
        };

        let mut buffer = Vec::new();
        write_listing_summary(&mut buffer, &result, "octo", "repo")
            .expect("should write listing summary");

        let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
        assert!(
            output.contains("Pull requests for octo/repo:"),
            "missing header: {output}"
        );
        assert!(
            output.contains("#42 [open] Add pagination (@octocat)"),
            "missing PR line: {output}"
        );
        assert!(
            output.contains("Page 2 of 3 (1 PRs shown)"),
            "missing page line: {output}"
        );
        assert!(
            output.contains("More pages available."),
            "missing next-page hint: {output}"
        );
    }

    #[test]
    fn write_listing_summary_defaults_total_pages_to_one_when_unknown() {
        let page_info = PageInfo::builder(1, 50).build();
        let result = PaginatedPullRequests {
            items: vec![],
            page_info,
            rate_limit: None,
        };

        let mut buffer = Vec::new();
        write_listing_summary(&mut buffer, &result, "octo", "repo")
            .expect("should write listing summary");

        let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
        assert!(
            output.contains("Page 1 of 1 (0 PRs shown)"),
            "expected default total pages of 1, got: {output}"
        );
    }

    #[test]
    fn write_pr_summary_to_includes_pr_details() {
        let details = PullRequestDetails {
            metadata: PullRequestMetadata {
                number: 123,
                title: Some("Fix critical bug".to_owned()),
                author: Some("contributor".to_owned()),
                html_url: Some("https://github.com/octo/cat/pull/123".to_owned()),
                ..Default::default()
            },
            comments: vec![],
        };

        let mut buffer = Vec::new();
        write_pr_summary_to(&mut buffer, &details).expect("should write PR summary");

        let output = String::from_utf8(buffer).expect("output should be valid UTF-8");
        assert!(
            output.contains("Loaded PR #123 by contributor"),
            "missing PR number and author: {output}"
        );
        assert!(
            output.contains("Fix critical bug"),
            "missing title: {output}"
        );
        assert!(
            output.contains("https://github.com/octo/cat/pull/123"),
            "missing URL: {output}"
        );
        assert!(
            output.contains("Comments: 0"),
            "missing comment count: {output}"
        );
    }
}
