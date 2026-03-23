#![allow(clippy::redundant_closure_call, clippy::derive_partial_eq_without_eq)]

mod expr;
pub use expr::*;
pub use jrsonnet_interner::IStr;
pub mod function;
mod location;
mod source;
pub mod unescape;
pub mod visit;

pub use location::CodeLocation;
pub use source::{
	Source, SourceDefaultIgnoreJpath, SourceDirectory, SourceFifo, SourceFile, SourcePath,
	SourcePathT, SourceVirtual,
};
