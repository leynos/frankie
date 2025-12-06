//! Frankie CLI entrypoint for pull request intake.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use frankie::{
    IntakeError, OctocrabGateway, PersonalAccessToken, PullRequestDetails, PullRequestIntake,
    PullRequestLocator,
};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if writeln!(io::stderr().lock(), "{error}").is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), IntakeError> {
    let args = parse_args()?;
    let locator = PullRequestLocator::parse(&args.pr_url)?;
    let token = PersonalAccessToken::new(args.token)?;

    let gateway = OctocrabGateway::for_token(&token, &locator)?;
    let intake = PullRequestIntake::new(&gateway);
    let details = intake.load(&locator).await?;

    write_summary(&details)?;
    Ok(())
}

struct CliArgs {
    pr_url: String,
    token: String,
}

fn parse_flag_value(
    args: &mut impl Iterator<Item = String>,
    error: IntakeError,
) -> Result<String, IntakeError> {
    args.next().ok_or(error)
}

fn parse_args() -> Result<CliArgs, IntakeError> {
    let mut pr_url: Option<String> = None;
    let mut token: Option<String> = env::var("GITHUB_TOKEN").ok();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--pr-url=") {
            pr_url = Some(value.to_owned());
            continue;
        }
        if let Some(value) = arg.strip_prefix("--token=") {
            token = Some(value.to_owned());
            continue;
        }
        match arg.as_str() {
            "--pr-url" | "-u" => {
                pr_url = Some(parse_flag_value(
                    &mut args,
                    IntakeError::MissingPullRequestUrl,
                )?);
            }
            "--token" | "-t" => {
                token = Some(parse_flag_value(&mut args, IntakeError::MissingToken)?);
            }
            _ => {
                return Err(IntakeError::InvalidArgument {
                    argument: arg.clone(),
                });
            }
        }
    }

    let pr_url_value = pr_url.ok_or(IntakeError::MissingPullRequestUrl)?;
    let token_value = token.ok_or(IntakeError::MissingToken)?;
    Ok(CliArgs {
        pr_url: pr_url_value,
        token: token_value,
    })
}

fn write_summary(details: &PullRequestDetails) -> Result<(), IntakeError> {
    let mut stdout = io::stdout().lock();
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

    writeln!(stdout, "{message}").map_err(|error| IntakeError::Api {
        message: error.to_string(),
    })
}
