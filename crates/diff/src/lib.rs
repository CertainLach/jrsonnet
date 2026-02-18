//! Shared diffing and similarity utilities.
//!
//! This crate centralizes output comparison logic used by CLI tools.

pub mod directory;
pub mod json;
pub mod output;
pub mod similarity;
pub mod string;
pub mod unified;

pub use similarity::SimilarityScore;
