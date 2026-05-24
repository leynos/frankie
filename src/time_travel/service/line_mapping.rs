//! Line-mapping helpers for time-travel service orchestration.

use metrics::counter;

use crate::local::{
    CommitSha, GitOperations, LineMappingRequest, LineMappingVerification, RepoFilePath,
};

use super::git_error_type;

#[derive(Debug, Clone, Copy)]
pub(super) struct LineMappingContext<'a> {
    pub(super) git_ops: &'a dyn GitOperations,
    pub(super) commit_sha: &'a CommitSha,
    pub(super) file_path: &'a RepoFilePath,
    pub(super) original_line: Option<u32>,
    pub(super) head_sha: Option<&'a CommitSha>,
}

pub(super) fn verify_line_mapping(
    context: &LineMappingContext<'_>,
) -> Option<LineMappingVerification> {
    let Some((line, head)) = context.original_line.zip(context.head_sha) else {
        log_skipped_line_mapping(context);
        return None;
    };
    let request = LineMappingRequest::new(
        context.commit_sha.as_str().to_owned(),
        head.as_str().to_owned(),
        context.file_path.as_str().to_owned(),
        line,
    );
    tracing::debug!(
        commit_sha = context.commit_sha.as_str(),
        head_sha = head.as_str(),
        file_path = context.file_path.as_str(),
        original_line = line,
        "verifying time-travel line mapping"
    );
    context
        .git_ops
        .verify_line_mapping(&request)
        .map_err(|error| {
            counter!(
                "time_travel_service_operation_errors_total",
                "operation" => "line_mapping",
                "error_type" => git_error_type(&error)
            )
            .increment(1);
            tracing::debug!(
                commit_sha = context.commit_sha.as_str(),
                head_sha = head.as_str(),
                file_path = context.file_path.as_str(),
                original_line = line,
                ?error,
                "time-travel line mapping verification failed"
            );
            error
        })
        .ok()
}

fn log_skipped_line_mapping(context: &LineMappingContext<'_>) {
    tracing::debug!(
        commit_sha = context.commit_sha.as_str(),
        file_path = context.file_path.as_str(),
        has_original_line = context.original_line.is_some(),
        has_head_sha = context.head_sha.is_some(),
        "skipping time-travel line mapping verification"
    );
}
