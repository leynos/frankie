//! Contextual error messages for time-travel navigation failures.
//!
//! Builds user-friendly, actionable error messages when time-travel is
//! attempted without a valid local repository. Includes PR-specific
//! guidance and resolution steps.

use std::fmt::Write;

use crate::tui::TimeTravelContext;

/// Builds a contextual error message for time-travel unavailability.
///
/// The message explains what time-travel needs, describes the current
/// situation (why it failed), and provides numbered steps for resolution.
///
/// # Arguments
///
/// * `context` - PR metadata and optional discovery failure reason.
///
/// # Example output
///
/// ```text
/// Time travel requires a local repository checkout.
///
/// The time-travel feature needs access to repository history to show
/// how files looked at different commits.
///
/// Current situation:
///   PR repository: owner/repo (PR #42)
///   Discovery: not inside a Git repository
///
/// To use time travel:
///   1. Clone the repository:
///      git clone https://ghe.corp.com/owner/repo
///   2. Fetch the PR branch:
///      git fetch origin pull/42/head:pr-42 && git checkout pr-42
///   3. Run frankie from within the repository directory
///
/// Alternatively, use --repo-path to specify your local checkout.
/// ```
#[must_use]
pub(crate) fn build_time_travel_error(context: &TimeTravelContext) -> String {
    let TimeTravelContext {
        host,
        owner,
        repo,
        pr_number,
        discovery_failure,
    } = context;

    let mut msg = String::from(concat!(
        "Time travel requires a local repository checkout.\n",
        "\n",
        "The time-travel feature needs access to repository history\n",
        "to show how files looked at different commits.\n",
    ));

    // Writing to String cannot fail.
    #[expect(
        clippy::let_underscore_must_use,
        reason = "write! to String cannot fail"
    )]
    let _ = write!(
        msg,
        "\nCurrent situation:\n  PR repository: {owner}/{repo} (PR #{pr_number})\n",
    );

    if let Some(reason) = discovery_failure {
        #[expect(
            clippy::let_underscore_must_use,
            reason = "writeln! to String cannot fail"
        )]
        let _ = writeln!(msg, "  Discovery: {reason}");
    }

    #[expect(
        clippy::let_underscore_must_use,
        reason = "write! to String cannot fail"
    )]
    let _ = write!(
        msg,
        concat!(
            "\n",
            "To use time travel:\n",
            "  1. Clone the repository:\n",
            "     git clone https://{host}/{owner}/{repo}\n",
            "  2. Fetch the PR branch:\n",
            "     git fetch origin pull/{pr_number}/head:pr-{pr_number}",
            " && git checkout pr-{pr_number}\n",
            "  3. Run frankie from within the repository directory\n",
            "\n",
            "Alternatively, use --repo-path to specify your local checkout.",
        ),
        host = host,
        owner = owner,
        repo = repo,
        pr_number = pr_number,
    );

    msg
}

/// Builds a fallback error message when no time-travel context is available.
///
/// This occurs when the TUI is used outside the normal startup flow
/// (e.g. in tests or embedded usage) and no context was stored.
#[must_use]
pub(crate) fn build_fallback_time_travel_error() -> String {
    String::from(concat!(
        "Time travel requires a local repository checkout.\n",
        "\n",
        "Use --repo-path to specify the location of your local\n",
        "repository checkout, or run frankie from within the\n",
        "repository directory.",
    ))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    /// Test expectation for a time-travel error message.
    struct ErrorExpectation {
        context: TimeTravelContext,
        expected: &'static [&'static str],
        unexpected: &'static [&'static str],
    }

    fn pr_metadata() -> ErrorExpectation {
        ErrorExpectation {
            context: TimeTravelContext {
                host: "github.com".to_owned(),
                owner: "octocat".to_owned(),
                repo: "hello-world".to_owned(),
                pr_number: 42,
                discovery_failure: None,
            },
            expected: &[
                "octocat/hello-world",
                "PR #42",
                "git clone https://github.com/octocat/hello-world",
                "pull/42/head:pr-42",
                "--repo-path",
            ],
            unexpected: &[],
        }
    }

    fn discovery_failure_reason() -> ErrorExpectation {
        ErrorExpectation {
            context: TimeTravelContext {
                host: "github.com".to_owned(),
                owner: "octocat".to_owned(),
                repo: "hello-world".to_owned(),
                pr_number: 7,
                discovery_failure: Some("not inside a Git repository".to_owned()),
            },
            expected: &["not inside a Git repository", "Discovery:"],
            unexpected: &[],
        }
    }

    fn no_failure_omits_discovery_line() -> ErrorExpectation {
        ErrorExpectation {
            context: TimeTravelContext {
                host: "github.com".to_owned(),
                owner: "octocat".to_owned(),
                repo: "hello-world".to_owned(),
                pr_number: 1,
                discovery_failure: None,
            },
            expected: &[],
            unexpected: &["Discovery:"],
        }
    }

    fn mismatch_reason() -> ErrorExpectation {
        ErrorExpectation {
            context: TimeTravelContext {
                host: "github.com".to_owned(),
                owner: "alice".to_owned(),
                repo: "project".to_owned(),
                pr_number: 99,
                discovery_failure: Some(
                    concat!(
                        "local repository origin (bob/other-project) does not ",
                        "match the PR repository (alice/project)"
                    )
                    .to_owned(),
                ),
            },
            expected: &["bob/other-project", "alice/project"],
            unexpected: &[],
        }
    }

    fn enterprise_host_in_clone_url() -> ErrorExpectation {
        ErrorExpectation {
            context: TimeTravelContext {
                host: "ghe.corp.com".to_owned(),
                owner: "team".to_owned(),
                repo: "project".to_owned(),
                pr_number: 5,
                discovery_failure: None,
            },
            expected: &["git clone https://ghe.corp.com/team/project"],
            unexpected: &[],
        }
    }

    #[rstest]
    #[case::pr_metadata(pr_metadata())]
    #[case::discovery_failure_reason(discovery_failure_reason())]
    #[case::no_failure_omits_discovery_line(no_failure_omits_discovery_line())]
    #[case::mismatch_reason(mismatch_reason())]
    #[case::enterprise_host_in_clone_url(enterprise_host_in_clone_url())]
    fn build_time_travel_error_content(#[case] expectation: ErrorExpectation) {
        let msg = build_time_travel_error(&expectation.context);

        for needle in expectation.expected {
            assert!(msg.contains(needle), "should contain {needle:?}: {msg}");
        }
        for needle in expectation.unexpected {
            assert!(
                !msg.contains(needle),
                "should not contain {needle:?}: {msg}"
            );
        }
    }

    #[test]
    fn fallback_error_mentions_repo_path() {
        let msg = build_fallback_time_travel_error();

        assert!(
            msg.contains("--repo-path"),
            "fallback should mention --repo-path: {msg}"
        );
        assert!(
            msg.contains("local repository checkout"),
            "fallback should explain requirement: {msg}"
        );
    }
}
