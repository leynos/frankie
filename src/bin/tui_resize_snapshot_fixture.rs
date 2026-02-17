//! Dedicated fixture binary for deterministic TUI snapshot testing.
//!
//! The fixture avoids network calls and injects a fixed set of review comments
//! so PTY-backed tests can validate terminal sizing and resize behaviour.

use bubbletea_rs::Program;
use crossterm::terminal;
use frankie::github::models::ReviewComment;
use frankie::tui::{ReviewApp, set_initial_reviews, set_initial_terminal_size};
use std::env;
use std::process;
use std::time::Duration;

fn fixture_comments() -> Vec<ReviewComment> {
    (1..=18)
        .map(|id| {
            let line_number = u32::try_from(id).unwrap_or(u32::MAX);
            let line = 10_u32.saturating_add(u32::try_from(id).unwrap_or_default().saturating_mul(3));
            let author = format!("reviewer-{id}");
            let mut file_suffix = id + 1;
            while file_suffix > 5 {
                file_suffix -= 5;
            }
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
        if let Some(value) = arg.strip_prefix("--auto-exit-ms=")
            && let Ok(duration) = value.parse::<u64>()
        {
            return Some(duration);
        }

        if arg == "--auto-exit-ms"
            && let Some(value) = args.next()
            && let Ok(duration) = value.parse::<u64>()
        {
            return Some(duration);
        }
    }

    None
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
    let result = match auto_exit_duration_ms() {
        Some(duration_ms) => tokio::time::timeout(Duration::from_millis(duration_ms), run_future)
            .await
            .unwrap_or_else(|_| Ok(ReviewApp::empty())),
        None => run_future.await,
    };

    if result.is_err() {
        process::exit(1);
    }
}
