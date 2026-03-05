//! Behavioural tests for resolution verification TUI flows.

use std::sync::Arc;

use bubbletea_rs::Cmd;
use bubbletea_rs::Model;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};
use tempfile::TempDir;

use frankie::github::models::ReviewComment;
use frankie::local::{
    CommitMetadata, CommitSha, CommitSnapshot, GitOperationError, GitOperations,
    LineMappingRequest, LineMappingVerification, RepoFilePath,
};
use frankie::persistence::{ReviewCommentVerificationCache, migrate_database};
use frankie::telemetry::NoopTelemetrySink;
use frankie::tui::app::ReviewApp;
use frankie::tui::messages::AppMsg;
use frankie::verification::{
    CommentVerificationEvidence, CommentVerificationEvidenceKind, CommentVerificationResult,
    CommentVerificationStatus, ResolutionVerificationService,
};

type StepResult = Result<(), Box<dyn std::error::Error>>;

#[derive(ScenarioState, Default)]
struct TuiVerifyState {
    temp_dir: Slot<TempDir>,
    app: Slot<ReviewApp>,
    pending_cmd: Slot<Option<Cmd>>,
    rendered_view: Slot<String>,
}

#[fixture]
fn tui_verify_state() -> TuiVerifyState {
    TuiVerifyState::default()
}

#[derive(Debug)]
struct StubVerifier {
    status: CommentVerificationStatus,
}

impl ResolutionVerificationService for StubVerifier {
    fn verify_comment(
        &self,
        comment: &ReviewComment,
        target_sha: &str,
    ) -> CommentVerificationResult {
        let kind = match self.status {
            CommentVerificationStatus::Verified => CommentVerificationEvidenceKind::LineChanged,
            CommentVerificationStatus::Unverified => CommentVerificationEvidenceKind::LineUnchanged,
        };
        CommentVerificationResult::new(
            comment.id,
            target_sha.to_owned(),
            self.status,
            CommentVerificationEvidence {
                kind,
                message: Some("stub".to_owned()),
            },
        )
    }
}

#[derive(Debug)]
struct NoopGitOps;

impl GitOperations for NoopGitOps {
    fn get_commit_snapshot(
        &self,
        sha: &CommitSha,
        _file_path: Option<&RepoFilePath>,
    ) -> Result<CommitSnapshot, GitOperationError> {
        let timestamp = chrono::Utc::now();
        let metadata = CommitMetadata::new(
            sha.to_string(),
            "message".to_owned(),
            "author".to_owned(),
            timestamp,
        );
        Ok(CommitSnapshot::new(metadata))
    }

    fn get_file_at_commit(
        &self,
        _sha: &CommitSha,
        _file_path: &RepoFilePath,
    ) -> Result<String, GitOperationError> {
        Err(GitOperationError::RepositoryNotAvailable {
            message: "noop".to_owned(),
        })
    }

    fn verify_line_mapping(
        &self,
        request: &LineMappingRequest,
    ) -> Result<LineMappingVerification, GitOperationError> {
        Ok(LineMappingVerification::exact(request.line))
    }

    fn get_parent_commits(
        &self,
        _sha: &CommitSha,
        _limit: usize,
    ) -> Result<Vec<CommitSha>, GitOperationError> {
        Ok(Vec::new())
    }

    fn commit_exists(&self, _sha: &CommitSha) -> bool {
        true
    }
}

fn sample_comment() -> ReviewComment {
    ReviewComment {
        id: 1,
        body: Some("Please address this".to_owned()),
        author: Some("alice".to_owned()),
        file_path: Some("src/main.rs".to_owned()),
        line_number: Some(2),
        original_line_number: Some(2),
        diff_hunk: None,
        commit_sha: Some("old".to_owned()),
        in_reply_to_id: None,
        created_at: None,
        updated_at: None,
    }
}

fn parse_status(text: &str) -> Result<CommentVerificationStatus, Box<dyn std::error::Error>> {
    match text.trim_matches('"') {
        "verified" => Ok(CommentVerificationStatus::Verified),
        "unverified" => Ok(CommentVerificationStatus::Unverified),
        other => Err(format!("unsupported status: {other}").into()),
    }
}

#[given("a review TUI with verification cache configured returning {text}")]
fn given_tui_with_cache(tui_verify_state: &TuiVerifyState, text: String) -> StepResult {
    let status = parse_status(&text)?;

    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("frankie.sqlite");
    let database_url = db_path.to_string_lossy().to_string();
    migrate_database(&database_url, &NoopTelemetrySink)?;
    let cache = ReviewCommentVerificationCache::new(database_url)?;

    let app = ReviewApp::new(vec![sample_comment()])
        .with_git_ops(Arc::new(NoopGitOps), "head".to_owned())
        .with_review_comment_verification_cache(Arc::new(cache))
        .with_resolution_verification_service(Arc::new(StubVerifier { status }));

    tui_verify_state.temp_dir.set(temp_dir);
    tui_verify_state.app.set(app);
    tui_verify_state.pending_cmd.set(None);
    Ok(())
}

#[given("a review TUI with no verification cache")]
fn given_tui_without_cache(tui_verify_state: &TuiVerifyState) {
    let status = CommentVerificationStatus::Verified;

    let app = ReviewApp::new(vec![sample_comment()])
        .with_git_ops(Arc::new(NoopGitOps), "head".to_owned())
        .with_resolution_verification_service(Arc::new(StubVerifier { status }));

    tui_verify_state.app.set(app);
    tui_verify_state.pending_cmd.set(None);
}

#[when("the user requests verification for the selected comment")]
fn when_user_requests_verification(tui_verify_state: &TuiVerifyState) -> StepResult {
    let maybe_cmd = tui_verify_state
        .app
        .with_mut(|app| app.handle_message(&AppMsg::VerifySelectedComment))
        .ok_or("app should be initialised")?;
    tui_verify_state.pending_cmd.set(maybe_cmd);
    Ok(())
}

#[when("the verification command completes")]
fn when_verification_cmd_completes(tui_verify_state: &TuiVerifyState) -> StepResult {
    let maybe_cmd = tui_verify_state
        .pending_cmd
        .with_mut(Option::take)
        .ok_or("pending command slot should be initialised")?;
    let cmd = maybe_cmd.ok_or("expected pending verification command")?;

    let runtime = tokio::runtime::Runtime::new()?;
    let maybe_msg = runtime.block_on(cmd);
    let Some(message) = maybe_msg else {
        return Err("verification command should return a message".into());
    };
    let app_msg = message
        .downcast::<AppMsg>()
        .map_err(|_| "verification command returned a non-AppMsg value")?;

    let view = tui_verify_state
        .app
        .with_mut(|app| {
            app.handle_message(&app_msg);
            app.view()
        })
        .ok_or("app should be initialised")?;
    tui_verify_state.rendered_view.set(view);
    Ok(())
}

#[then("the review list shows the comment as verified")]
fn then_list_shows_verified(tui_verify_state: &TuiVerifyState) -> StepResult {
    let view = tui_verify_state
        .rendered_view
        .get()
        .ok_or("rendered view should be captured")?;
    assert!(
        view.contains(">✓ [alice]"),
        "expected selected row to include verified marker, got:\n{view}"
    );
    Ok(())
}

#[then("an error is shown explaining the missing database")]
fn then_error_is_shown(tui_verify_state: &TuiVerifyState) -> StepResult {
    let view = tui_verify_state
        .app
        .with_ref(Model::view)
        .ok_or("app should be initialised")?;
    assert!(
        view.contains("--database-url"),
        "expected error to mention --database-url, got:\n{view}"
    );
    Ok(())
}

#[scenario(
    path = "tests/features/tui_verify_resolutions.feature",
    name = "Verifying a selected comment annotates the review list"
)]
fn scenario_verify_selected_comment(tui_verify_state: TuiVerifyState) {
    let _ = tui_verify_state;
}

#[scenario(
    path = "tests/features/tui_verify_resolutions.feature",
    name = "Verification requires a configured cache"
)]
fn scenario_verify_requires_cache(tui_verify_state: TuiVerifyState) {
    let _ = tui_verify_state;
}
