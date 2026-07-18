//! Shared PR-discussion summary APIs used by CLI and TUI adapters.

mod deep_link;
mod model;
mod openai;
mod service;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
mod threads;

pub use deep_link::FrankieDeepLink;
pub use model::{
    DiscussionSeverity, DiscussionSummaryItem, FileDiscussionSummary, PrDiscussionSummary,
    PrDiscussionSummaryRequest, ReviewView, ReviewViewRef, SeverityBucket,
};
pub use openai::{OpenAiPrDiscussionSummaryConfig, OpenAiPrDiscussionSummaryService};
pub use service::PrDiscussionSummaryService;
