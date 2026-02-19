//! Dedicated fixture binary for deterministic TUI snapshot testing.
//!
//! The fixture avoids network calls and injects a fixed set of review comments
//! so PTY-backed tests can validate terminal sizing and resize behaviour.

use bubbletea_rs::Program;
use crossterm::terminal;
use frankie::github::models::ReviewComment;
use frankie::tui::{ReviewApp, set_initial_reviews, set_initial_terminal_size};
use std::env;
use std::future::Future;
use std::process;
use std::time::Duration;

fn fixture_comments() -> Vec<ReviewComment> {
    (1_u64..=18)
        .map(|id| {
            let line_number = u32::try_from(id).unwrap_or(u32::MAX);
            let line = 10_u32.saturating_add(u32::try_from(id).unwrap_or_default().saturating_mul(3));
            let author = format!("reviewer-{id}");
            let file_suffix = id.rem_euclid(5) + 1;
            let file_path = format!("src/component_{file_suffix:02}.rs");
            let body = format!("Fixture review {id}: adjust layout and confirm visibility");
            let diff_hunk = format!(
                "@@ -{line},1 +{line},3 @@\n+pub fn review_{id}() {{\n+    println!(\"review {id}\");\n+}}"
            );

            ReviewComment {
                id,
                body: Some(body),
                author: Some(author),
                file_path: Some(file_path),
                line_number: Some(line_number),
                diff_hunk: Some(diff_hunk),
                ..Default::default()
            }
        })
        .collect()
}

fn seed_initial_terminal_size() {
    if let Ok((width, height)) = terminal::size() {
        let _ = set_initial_terminal_size(width, height);
    }
}

fn auto_exit_duration_ms() -> Option<u64> {
    parse_auto_exit_duration_ms_from_args().or_else(|| {
        env::var("TUI_RESIZE_FIXTURE_AUTO_EXIT_MS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
    })
}

fn parse_auto_exit_duration_ms_from_args() -> Option<u64> {
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        if let Some(duration) = arg
            .strip_prefix("--auto-exit-ms=")
            .and_then(|value| value.parse::<u64>().ok())
        {
            return Some(duration);
        }

        if arg == "--auto-exit-ms"
            && let Some(duration) = args.next().and_then(|value| value.parse::<u64>().ok())
        {
            return Some(duration);
        }
    }

    None
}

#[derive(Debug, PartialEq, Eq)]
enum FixtureRunFailure {
    TimedOut { duration_ms: u64 },
}

async fn await_with_optional_timeout<F, T>(
    run_future: F,
    duration_ms: Option<u64>,
) -> std::result::Result<T, FixtureRunFailure>
where
    F: Future<Output = T>,
{
    match duration_ms {
        Some(timeout_ms) => tokio::time::timeout(Duration::from_millis(timeout_ms), run_future)
            .await
            .map_err(|_| FixtureRunFailure::TimedOut {
                duration_ms: timeout_ms,
            }),
        None => Ok(run_future.await),
    }
}

#[tokio::main]
async fn main() {
    let _ = set_initial_reviews(fixture_comments());
    seed_initial_terminal_size();

    let program_builder = Program::<ReviewApp>::builder().alt_screen(true);
    let Ok(program) = program_builder.build() else {
        process::exit(1);
    };

    let run_future = program.run();
    let run_result = match await_with_optional_timeout(run_future, auto_exit_duration_ms()).await {
        Ok(result) => result,
        Err(FixtureRunFailure::TimedOut { .. }) => process::exit(1),
    };

    if run_result.is_err() {
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::future::{pending, ready};

    use super::{FixtureRunFailure, await_with_optional_timeout};

    #[tokio::test]
    async fn await_with_optional_timeout_returns_output_without_timeout() {
        let result = await_with_optional_timeout(ready(42_u8), None).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn await_with_optional_timeout_returns_output_within_timeout() {
        let result = await_with_optional_timeout(ready(7_u8), Some(50)).await;
        assert_eq!(result, Ok(7));
    }

    #[tokio::test]
    async fn await_with_optional_timeout_errors_when_future_hangs() {
        let result = await_with_optional_timeout(pending::<u8>(), Some(1)).await;
        assert_eq!(result, Err(FixtureRunFailure::TimedOut { duration_ms: 1 }));
    }
}
