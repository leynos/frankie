//! Resolution verification handlers.
//!
//! Provides message handlers for verifying review comments against local git
//! state and updating the UI with verified/unverified annotations.

use std::any::Any;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bubbletea_rs::Cmd;

use crate::persistence::{PersistenceError, ReviewCommentVerificationCacheWrite};
use crate::tui::messages::AppMsg;
use crate::verification::CommentVerificationResult;

use super::ReviewApp;

#[derive(Debug)]
struct VerifyTaskParams {
    request_id: u64,
    service: Arc<dyn crate::verification::ResolutionVerificationService>,
    cache: Arc<crate::persistence::ReviewCommentVerificationCache>,
    target_sha: String,
    comments: Vec<crate::github::models::ReviewComment>,
}

impl ReviewApp {
    pub(super) fn handle_verification_msg(&mut self, msg: &AppMsg) -> Option<Cmd> {
        match msg {
            AppMsg::VerifySelectedComment => self.handle_verify_selected_comment(),
            AppMsg::VerifyFilteredComments => self.handle_verify_filtered_comments(),
            AppMsg::VerificationReady {
                request_id,
                results,
                persistence_error,
            } => self.handle_verification_ready(*request_id, results, persistence_error.as_deref()),
            AppMsg::VerificationFailed {
                request_id,
                message,
            } => self.handle_verification_failed(*request_id, message),
            _ => None,
        }
    }

    fn handle_verify_selected_comment(&mut self) -> Option<Cmd> {
        let comment = self.selected_comment()?.clone();
        self.spawn_verification(vec![comment])
    }

    fn handle_verify_filtered_comments(&mut self) -> Option<Cmd> {
        let comments: Vec<_> = self
            .filtered_indices
            .iter()
            .filter_map(|&i| self.reviews.get(i).cloned())
            .collect();
        if comments.is_empty() {
            self.error = Some("No comments to verify for the current filter".to_owned());
            return None;
        }
        self.spawn_verification(comments)
    }

    fn spawn_verification(
        &mut self,
        comments: Vec<crate::github::models::ReviewComment>,
    ) -> Option<Cmd> {
        let Some(service) = self.resolution_verification_service.as_ref() else {
            self.error = Some(
                "Resolution verification requires a local repository (no git operations available)"
                    .to_owned(),
            );
            return None;
        };
        let Some(cache) = self.review_comment_verification_cache.as_ref() else {
            self.error = Some(
                "Resolution verification requires --database-url to persist results".to_owned(),
            );
            return None;
        };
        let Some(target_sha) = self.head_sha.as_deref() else {
            self.error = Some("Resolution verification requires a HEAD SHA".to_owned());
            return None;
        };

        let request_id = self.next_verification_request_id;
        self.next_verification_request_id = self.next_verification_request_id.saturating_add(1);
        self.in_flight_verification_request_id = Some(request_id);

        let verification_service = Arc::clone(service);
        let verification_cache = Arc::clone(cache);

        Some(spawn_verify_task(VerifyTaskParams {
            request_id,
            service: verification_service,
            cache: verification_cache,
            target_sha: target_sha.to_owned(),
            comments,
        }))
    }

    fn handle_verification_ready(
        &mut self,
        request_id: u64,
        results: &[CommentVerificationResult],
        persistence_error: Option<&str>,
    ) -> Option<Cmd> {
        if self.in_flight_verification_request_id != Some(request_id) {
            return None;
        }
        self.in_flight_verification_request_id = None;

        for result in results {
            self.review_comment_verifications
                .insert(result.github_comment_id(), result.clone());
        }

        if let Some(message) = persistence_error {
            self.error = Some(message.to_owned());
        }

        None
    }

    fn handle_verification_failed(&mut self, request_id: u64, message: &str) -> Option<Cmd> {
        if self.in_flight_verification_request_id != Some(request_id) {
            return None;
        }
        self.in_flight_verification_request_id = None;
        self.error = Some(message.to_owned());
        None
    }
}

fn spawn_verify_task(params: VerifyTaskParams) -> Cmd {
    Box::pin(async move {
        let result = tokio::task::spawn_blocking(move || {
            let results = params
                .service
                .verify_comments(&params.comments, &params.target_sha);
            let now_unix = unix_now();
            let persistence_error = persist_results(&params.cache, &results, now_unix).err();
            (results, persistence_error)
        })
        .await;

        match result {
            Ok((results, persistence_error)) => Some(Box::new(AppMsg::VerificationReady {
                request_id: params.request_id,
                results,
                persistence_error,
            }) as Box<dyn Any + Send>),
            Err(error) => Some(Box::new(AppMsg::VerificationFailed {
                request_id: params.request_id,
                message: format!("Task join error: {error}"),
            }) as Box<dyn Any + Send>),
        }
    })
}

fn persist_results(
    cache: &crate::persistence::ReviewCommentVerificationCache,
    results: &[CommentVerificationResult],
    now_unix: i64,
) -> Result<(), String> {
    for result in results {
        cache
            .upsert(ReviewCommentVerificationCacheWrite {
                result,
                verified_at_unix: now_unix,
            })
            .map_err(|error| map_persistence_error(&error))?;
    }
    Ok(())
}

fn map_persistence_error(error: &PersistenceError) -> String {
    format!("failed to persist verification results: {error}")
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}
