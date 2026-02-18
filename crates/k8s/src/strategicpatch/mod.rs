//! Strategic merge patch implementation for Kubernetes resources.
//!
//! This module implements kubectl's three-way strategic merge patch algorithm,
//! which properly handles:
//! - Array merging by merge key (e.g., `name` for containers)
//! - Strategic merge directives (`$patch`, `$setElementOrder`, `$deleteFromPrimitiveList`)
//! - Proper three-way diff using (original, modified, current)
//!
//! # Algorithm Overview
//!
//! The three-way merge patch algorithm works as follows:
//!
//! ```text
//! 1. delta = diff(current → modified, ignore_deletions=true)
//!    # What needs to change from current state to reach modified
//!
//! 2. deletions = diff(original → modified, ignore_changes_and_additions=true)
//!    # What fields were intentionally removed by user
//!
//! 3. patch = merge(deletions, delta)
//!    # Combine: apply user's deletions + changes from current
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use rtk::k8s::strategicpatch::{create_three_way_merge_patch, SchemaLookup};
//!
//! let patch = create_three_way_merge_patch(
//!     original.as_ref(),  // Last applied configuration (optional)
//!     &modified,          // Desired state from manifest
//!     &current,           // Current state from cluster
//!     &schema_lookup,     // Schema provider for merge keys
//! )?;
//! ```

pub mod builtin_schemas;
pub mod diff;
pub mod directives;
pub mod merge;
pub mod schema;
pub mod types;

pub use builtin_schemas::{get_merge_keys, gvk_to_schema_type, MergeType, BUILTIN_SCHEMAS};
pub use diff::{
	create_three_way_merge_patch, create_three_way_merge_patch_with_options, diff_values,
	require_key_unchanged, require_metadata_key_unchanged, DiffError, DiffOptions,
	PreconditionFunc,
};
pub use directives::{
	is_directive, strip_directives, DIRECTIVE_DELETE_FROM_PRIMITIVE_LIST_PREFIX, DIRECTIVE_PATCH,
	DIRECTIVE_RETAIN_KEYS, DIRECTIVE_SET_ELEMENT_ORDER_PREFIX,
};
pub use merge::{merge_patches, MergeOptions};
pub use schema::{BuiltinSchemaLookup, CombinedSchemaLookup, SchemaLookup};
pub use types::{FieldSchema, PatchMeta, PatchStrategy, TypeSchema};
