//! Lint check identifiers (for diagnostics and config).

/// Known lint check IDs. Used in diagnostics and to enable/disable checks.
pub const UNUSED_LOCALS: &str = "unused_locals";

/// All valid check IDs. Used to validate --disable-checks.
pub const ALL_CHECKS: &[&str] = &[UNUSED_LOCALS];
