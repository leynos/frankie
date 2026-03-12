//! Shared PR-discussion summary APIs used by CLI and TUI adapters.

mod model;
mod openai;
mod service;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
mod threads;

pub use model::{
    DiscussionSeverity, DiscussionSummaryItem, FileDiscussionSummary, PrDiscussionSummary,
    PrDiscussionSummaryRequest, SeverityBucket, TuiView, TuiViewLink,
};
pub use openai::{OpenAiPrDiscussionSummaryConfig, OpenAiPrDiscussionSummaryService};
pub use service::PrDiscussionSummaryService;
