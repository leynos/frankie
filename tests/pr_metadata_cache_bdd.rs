//! Behavioural tests for pull request metadata caching.

mod support {
    #[path = "../support/mod.rs"]
    mod common;

    pub use common::create_temp_dir;

    #[path = "../support/pr_metadata_cache_helpers.rs"]
    pub mod pr_metadata_cache_helpers;
    #[path = "../support/runtime.rs"]
    pub mod runtime;
}

#[path = "pr_metadata_cache_bdd/given.rs"]
mod pr_metadata_cache_bdd_given;
#[path = "pr_metadata_cache_bdd/state.rs"]
mod pr_metadata_cache_bdd_state;
#[path = "pr_metadata_cache_bdd/then.rs"]
mod pr_metadata_cache_bdd_then;
#[path = "pr_metadata_cache_bdd/when.rs"]
mod pr_metadata_cache_bdd_when;

use rstest::fixture;
use rstest_bdd_macros::scenario;

use pr_metadata_cache_bdd_state::CacheState;

#[fixture]
fn cache_state() -> CacheState {
    CacheState::default()
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 0)]
fn fresh_cache_avoids_refetch(cache_state: CacheState) {
    let _ = cache_state;
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 1)]
fn expired_cache_revalidates(cache_state: CacheState) {
    let _ = cache_state;
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 2)]
fn changed_etag_invalidates(cache_state: CacheState) {
    let _ = cache_state;
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 3)]
fn cache_requires_schema(cache_state: CacheState) {
    let _ = cache_state;
}

#[scenario(path = "tests/features/pr_metadata_cache.feature", index = 4)]
fn uncached_not_modified_returns_error(cache_state: CacheState) {
    let _ = cache_state;
}
