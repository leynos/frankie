//! Review comment verification cache backed by `SQLite`.
//!
//! Automated resolution verification produces per-comment verdicts when a
//! comment is verified against a target commit SHA (typically the current
//! repository `HEAD`). This module persists those verdicts so subsequent runs
//! can annotate comments as verified/unverified without recomputing results.

use std::collections::{HashMap, HashSet};

use diesel::Connection;
use diesel::QueryableByName;
use diesel::RunQueryDsl;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Nullable, Text};
use diesel::sqlite::SqliteConnection;

use crate::persistence::PersistenceError;
use crate::verification::{
    CommentVerificationEvidenceKind, CommentVerificationResult, CommentVerificationStatus,
    GithubCommentId,
};

const REVIEW_COMMENT_VERIFICATIONS_TABLE: &str = "review_comment_verifications";

#[derive(Debug, QueryableByName)]
struct VerificationRow {
    #[diesel(sql_type = BigInt)]
    github_comment_id: i64,
    #[diesel(sql_type = Text)]
    target_sha: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Text)]
    evidence_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    evidence_message: Option<String>,
    #[diesel(sql_type = BigInt)]
    verified_at_unix: i64,
}

/// Cached verification result for a review comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedReviewCommentVerification {
    /// GitHub review comment identifier.
    pub github_comment_id: u64,
    /// Target commit SHA used for verification (typically HEAD).
    pub target_sha: String,
    /// Verified/unverified status.
    pub status: CommentVerificationStatus,
    /// Evidence kind explaining why the status was chosen.
    pub evidence_kind: CommentVerificationEvidenceKind,
    /// Optional evidence detail for display.
    pub evidence_message: Option<String>,
    /// Unix timestamp when verification was performed.
    pub verified_at_unix: i64,
}

/// SQLite-backed cache for review comment verification results.
#[derive(Debug, Clone)]
pub struct ReviewCommentVerificationCache {
    database_url: String,
}

/// Data required to insert or update a verification cache row.
#[derive(Debug, Clone, Copy)]
pub struct ReviewCommentVerificationCacheWrite<'a> {
    /// Verification result to persist.
    pub result: &'a CommentVerificationResult,
    /// Unix timestamp when verification was performed.
    pub verified_at_unix: i64,
}

impl ReviewCommentVerificationCache {
    /// Creates a cache wrapper targeting the configured `database_url`.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError::BlankDatabaseUrl`] when the URL is blank.
    pub fn new(database_url: impl Into<String>) -> Result<Self, PersistenceError> {
        let database_url_string = database_url.into();
        if database_url_string.trim().is_empty() {
            return Err(PersistenceError::BlankDatabaseUrl);
        }
        Ok(Self {
            database_url: database_url_string,
        })
    }

    /// Fetch cached verification results for the given comment IDs at
    /// `target_sha`.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when the schema is missing or the query
    /// fails.
    pub fn get_for_comments(
        &self,
        github_comment_ids: &[u64],
        target_sha: &str,
    ) -> Result<HashMap<u64, CachedReviewCommentVerification>, PersistenceError> {
        if github_comment_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let requested_ids: HashSet<i64> = github_comment_ids
            .iter()
            .map(|id| Self::try_query_comment_id((*id).into()))
            .collect::<Result<_, _>>()?;

        let mut connection = self.establish_connection()?;

        let query = format!(
            concat!(
                "SELECT github_comment_id, target_sha, status, evidence_kind, ",
                "evidence_message, verified_at_unix ",
                "FROM {} ",
                "WHERE target_sha = ? AND github_comment_id = ?;"
            ),
            REVIEW_COMMENT_VERIFICATIONS_TABLE,
        );

        let mut rows = Vec::with_capacity(requested_ids.len());
        for github_comment_id in requested_ids {
            let mut comment_rows: Vec<VerificationRow> = sql_query(&query)
                .bind::<Text, _>(target_sha)
                .bind::<BigInt, _>(github_comment_id)
                .load(&mut connection)
                .map_err(|error| Self::map_query_error(&mut connection, &error))?;
            rows.append(&mut comment_rows);
        }

        let mut out = HashMap::with_capacity(rows.len());
        for row in rows {
            let Ok(id_u64) = u64::try_from(row.github_comment_id) else {
                continue;
            };

            let Some(status) = CommentVerificationStatus::from_db_value(&row.status) else {
                continue;
            };
            let Some(evidence_kind) =
                CommentVerificationEvidenceKind::from_db_value(&row.evidence_kind)
            else {
                continue;
            };

            out.insert(
                id_u64,
                CachedReviewCommentVerification {
                    github_comment_id: id_u64,
                    target_sha: row.target_sha,
                    status,
                    evidence_kind,
                    evidence_message: row.evidence_message,
                    verified_at_unix: row.verified_at_unix,
                },
            );
        }

        Ok(out)
    }

    /// Inserts or updates a cached verification result.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when the schema is missing or the write
    /// fails.
    pub fn upsert(
        &self,
        write: ReviewCommentVerificationCacheWrite<'_>,
    ) -> Result<(), PersistenceError> {
        let mut connection = self.establish_connection()?;
        Self::upsert_with_connection(&mut connection, write)
    }

    fn upsert_with_connection(
        connection: &mut SqliteConnection,
        write: ReviewCommentVerificationCacheWrite<'_>,
    ) -> Result<(), PersistenceError> {
        let result = write.result;
        let evidence = result.evidence();
        let github_comment_id = Self::try_github_comment_id(result.github_comment_id())?;
        let query = format!(
            concat!(
                "INSERT INTO {} ",
                "(github_comment_id, target_sha, status, evidence_kind, evidence_message, ",
                "verified_at_unix) ",
                "VALUES (?, ?, ?, ?, ?, ?) ",
                "ON CONFLICT(github_comment_id, target_sha) DO UPDATE SET ",
                "status = excluded.status, ",
                "evidence_kind = excluded.evidence_kind, ",
                "evidence_message = excluded.evidence_message, ",
                "verified_at_unix = excluded.verified_at_unix, ",
                "updated_at = CURRENT_TIMESTAMP;"
            ),
            REVIEW_COMMENT_VERIFICATIONS_TABLE,
        );

        sql_query(query)
            .bind::<BigInt, _>(github_comment_id)
            .bind::<Text, _>(result.target_sha())
            .bind::<Text, _>(result.status().as_db_value())
            .bind::<Text, _>(evidence.kind.as_db_value())
            .bind::<Nullable<Text>, _>(evidence.message.as_deref())
            .bind::<BigInt, _>(write.verified_at_unix)
            .execute(connection)
            .map(drop)
            .map_err(|error| Self::map_write_error(connection, &error))
    }

    /// Inserts or updates multiple cached verification results.
    ///
    /// # Errors
    ///
    /// Returns [`PersistenceError`] when any write fails.
    pub fn upsert_all(
        &self,
        results: &[CommentVerificationResult],
        verified_at_unix: i64,
    ) -> Result<(), PersistenceError> {
        let mut connection = self.establish_connection()?;
        sql_query("BEGIN IMMEDIATE TRANSACTION;")
            .execute(&mut connection)
            .map(drop)
            .map_err(|error| Self::map_write_error(&mut connection, &error))?;

        for result in results {
            let write_result = Self::upsert_with_connection(
                &mut connection,
                ReviewCommentVerificationCacheWrite {
                    result,
                    verified_at_unix,
                },
            );
            if let Err(error) = write_result {
                drop(sql_query("ROLLBACK;").execute(&mut connection));
                return Err(error);
            }
        }

        sql_query("COMMIT;")
            .execute(&mut connection)
            .map(drop)
            .map_err(|error| Self::map_write_error(&mut connection, &error))
    }

    fn try_query_comment_id(github_comment_id: GithubCommentId) -> Result<i64, PersistenceError> {
        i64::try_from(github_comment_id.as_u64()).map_err(|_| PersistenceError::QueryFailed {
            message: format!(
                "github_comment_id {} exceeds i64 range",
                github_comment_id.as_u64()
            ),
        })
    }

    fn try_github_comment_id(github_comment_id: GithubCommentId) -> Result<i64, PersistenceError> {
        i64::try_from(github_comment_id.as_u64()).map_err(|_| PersistenceError::WriteFailed {
            message: format!(
                "github_comment_id {} exceeds i64 range",
                github_comment_id.as_u64()
            ),
        })
    }

    fn establish_connection(&self) -> Result<SqliteConnection, PersistenceError> {
        let mut connection = SqliteConnection::establish(&self.database_url).map_err(|error| {
            PersistenceError::ConnectionFailed {
                message: error.to_string(),
            }
        })?;

        sql_query("PRAGMA foreign_keys = ON;")
            .execute(&mut connection)
            .map(drop)
            .map_err(|error| PersistenceError::ForeignKeysEnableFailed {
                message: error.to_string(),
            })?;

        Ok(connection)
    }

    fn map_diesel_error<F>(
        connection: &mut SqliteConnection,
        error: &diesel::result::Error,
        error_constructor: F,
    ) -> PersistenceError
    where
        F: FnOnce(String) -> PersistenceError,
    {
        match Self::cache_table_exists(connection) {
            Ok(false) => PersistenceError::SchemaNotInitialised,
            Ok(true) | Err(_) => error_constructor(error.to_string()),
        }
    }

    fn map_query_error(
        connection: &mut SqliteConnection,
        error: &diesel::result::Error,
    ) -> PersistenceError {
        Self::map_diesel_error(connection, error, |message| PersistenceError::QueryFailed {
            message,
        })
    }

    fn map_write_error(
        connection: &mut SqliteConnection,
        error: &diesel::result::Error,
    ) -> PersistenceError {
        Self::map_diesel_error(connection, error, |message| PersistenceError::WriteFailed {
            message,
        })
    }

    fn cache_table_exists(
        connection: &mut SqliteConnection,
    ) -> Result<bool, diesel::result::Error> {
        #[derive(Debug, QueryableByName)]
        struct Count {
            #[diesel(sql_type = BigInt)]
            count: i64,
        }

        let count: Count = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = ?;",
        )
        .bind::<Text, _>(REVIEW_COMMENT_VERIFICATIONS_TABLE)
        .get_result(connection)?;

        Ok(count.count > 0)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
