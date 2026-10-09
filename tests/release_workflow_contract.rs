//! Contract for the release workflow. It builds on a stable toolchain with `cross +stable`, against a
//! `.cargo/config.toml` whose `rustflags` carry the nightly-only `-Zthreads` flag and, on Linux, the
//! mold linker flag. Cargo applies an assigned `RUSTFLAGS` instead of the configuration's, so every step
//! of that workflow that runs Cargo or `cross` must assign its own `RUSTFLAGS`, naming neither flag. The
//! fixtures come first, so the rule does not pass by reading nothing.

/// The release workflow, as the file is checked in.
const RELEASE_WORKFLOW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.github/workflows/release.yml"
));

/// The flag fragments a step may not assign: the nightly frontend flag and the mold linker flag.
const FORBIDDEN: [&str; 2] = ["-Zthreads", "mold"];

/// The text of a workflow file.
#[derive(Clone, Copy)]
struct Workflow<'a>(&'a str);

/// One step of a job: the lines from its list marker to the next step.
struct Step<'a> {
    lines: Vec<&'a str>,
}

impl<'a> Workflow<'a> {
    /// Splits the file into steps: each starts at a `- ` marker at the indentation of the first one
    /// under a `steps:` key, and runs to the next marker at that indentation.
    fn steps(self) -> Vec<Step<'a>> {
        let mut steps: Vec<Step<'a>> = Vec::new();
        let mut marker: Option<usize> = None;
        let mut in_steps = false;
        for line in self.0.lines() {
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim_start();
            let begins_step =
                in_steps && trimmed.starts_with("- ") && marker.is_none_or(|at| at == indent);
            if trimmed == "steps:" {
                in_steps = true;
                marker = None;
            } else if begins_step {
                marker = Some(indent);
                steps.push(Step { lines: vec![line] });
            } else if let Some(step) = steps.last_mut().filter(|_| in_steps) {
                step.lines.push(line);
            }
        }
        steps
    }
}

impl Step<'_> {
    /// Returns the step's `name:`, or the empty string.
    fn name(&self) -> &str {
        self.lines
            .iter()
            .find_map(|line| {
                line.trim_start()
                    .trim_start_matches("- ")
                    .strip_prefix("name:")
            })
            .map_or("", str::trim)
    }

    /// Returns the command lines of the step's `run:` key.
    fn commands(&self) -> Vec<&str> {
        self.lines
            .iter()
            .filter_map(|line| {
                line.trim_start()
                    .trim_start_matches("- ")
                    .strip_prefix("run:")
            })
            .map(str::trim)
            .collect()
    }

    /// Returns whether the step runs Cargo or `cross`.
    fn runs_cargo(&self) -> bool {
        self.commands()
            .iter()
            .any(|command| command.starts_with("cargo ") || command.starts_with("cross "))
    }

    /// Returns the value the step assigns to `RUSTFLAGS` in its `env:`, if it does.
    fn rustflags(&self) -> Option<&str> {
        self.lines
            .iter()
            .find_map(|line| line.trim_start().strip_prefix("RUSTFLAGS:"))
            .map(|value| value.trim().trim_matches(|c| c == '"' || c == '\''))
    }
}

/// Returns the complaints about a workflow: each step that runs Cargo or `cross` must assign a
/// `RUSTFLAGS` that names neither fast flag, and a `Build release binary` step running `cross +stable
/// build --release` must exist.
fn release_problems(workflow: Workflow<'_>) -> Vec<String> {
    let steps = workflow.steps();
    let mut problems = Vec::new();
    for step in steps.iter().filter(|step| step.runs_cargo()) {
        let name = step.name();
        match step.rustflags() {
            None => problems.push(format!(
                "step `{name}` runs Cargo without assigning RUSTFLAGS"
            )),
            Some(flags) => problems.extend(
                FORBIDDEN
                    .iter()
                    .filter(|fragment| flags.contains(**fragment))
                    .map(|fragment| format!("step `{name}` assigns RUSTFLAGS naming `{fragment}`")),
            ),
        }
    }
    let builds = steps
        .iter()
        .filter(|step| step.name() == "Build release binary")
        .filter(|step| {
            step.commands()
                .iter()
                .any(|command| command.starts_with("cross +stable build --release"))
        })
        .count();
    if builds != 1 {
        problems.push(format!("expected one `Build release binary` step running `cross +stable build --release`, found {builds}"));
    }
    problems
}

#[test]
fn the_release_workflow_assigns_rustflags_to_every_cargo_step() -> Result<(), String> {
    let problems = release_problems(Workflow(RELEASE_WORKFLOW));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("{problems:#?}"))
    }
}

mod fixtures {
    //! The rule over fixtures: a compliant release job passes, and each way of losing the assignment, naming
    //! a forbidden flag, losing the build step or reading nothing is refused.

    use super::{Workflow, release_problems};

    /// A job whose two Cargo steps assign a clean `RUSTFLAGS`.
    const COMPLIANT: &str = "\
jobs:
  build:
    steps:
      - uses: actions/checkout@v7
      - name: Install cross
        env:
          RUSTFLAGS: -D warnings
        run: cargo install cross
      - name: Build release binary
        env:
          RUSTFLAGS: -D warnings
        run: cross +stable build --release --target x86_64-unknown-linux-gnu
";

    fn found(text: &str) -> usize {
        release_problems(Workflow(text)).len()
    }

    #[test]
    fn a_compliant_job_passes() {
        assert_eq!(found(COMPLIANT), 0);
    }

    #[test]
    fn a_cargo_step_without_an_assignment_is_refused() {
        let install = COMPLIANT.replacen(
            "        env:\n          RUSTFLAGS: -D warnings\n        run: cargo install",
            "        run: cargo install",
            1,
        );
        assert_eq!(found(&install), 1);
        let build = COMPLIANT.replace(
            "        env:\n          RUSTFLAGS: -D warnings\n        run: cross",
            "        run: cross",
        );
        assert_eq!(found(&build), 1);
    }

    #[test]
    fn a_forbidden_flag_in_the_assignment_is_refused() {
        assert_eq!(
            found(&COMPLIANT.replacen("-D warnings", "-D warnings -Zthreads=8", 1)),
            1
        );
        assert_eq!(
            found(&COMPLIANT.replacen("-D warnings", "-D warnings -Clink-arg=-fuse-ld=mold", 1)),
            1
        );
    }

    #[test]
    fn a_missing_or_changed_build_step_is_refused() {
        assert_eq!(
            found(&COMPLIANT.replace("Build release binary", "Build")),
            1
        );
        assert_eq!(
            found(&COMPLIANT.replace("cross +stable build --release", "cross build")),
            1
        );
    }

    #[test]
    fn a_workflow_with_no_steps_is_refused() {
        assert_eq!(found("jobs: {}\n"), 1);
        assert_eq!(found(""), 1);
    }
}
