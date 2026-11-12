//! Xiezhi domain models, scoring, and shared types.

pub mod entity;
pub mod models;
pub mod scoring;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use entity::{MergeRequestReviewEntity, PushReviewEntity};
pub use models::{
    FileReviewResult, ScrComment, SEVERITY_EMOJI, SEVERITY_PENALTY, VALID_CATEGORIES,
    VALID_SEVERITIES,
};
pub use scoring::{score_grade, score_results, unified_score};
