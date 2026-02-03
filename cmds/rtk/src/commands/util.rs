//! Utilities for command handlers.

use std::io::{self, ErrorKind, Write};

use tracing::warn;

/// Warn about unimplemented CLI arguments that are accepted for Tanka compatibility
/// but don't do anything in Rustanka.
pub struct UnimplementedArgs<'a> {
	pub jsonnet_implementation: Option<&'a str>,
	pub cache_envs: Option<&'a [String]>,
	pub cache_path: Option<&'a Option<String>>,
	pub mem_ballast_size_bytes: Option<&'a Option<i64>>,
}

impl<'a> UnimplementedArgs<'a> {
	/// Log warnings for any unimplemented arguments that were provided.
	pub fn warn_if_set(&self) {
		if let Some(impl_str) = self.jsonnet_implementation {
			if impl_str != "go" {
				warn!(
					"--jsonnet-implementation is unimplemented in rtk and has no effect; \
					 rtk always uses the built-in jrsonnet evaluator"
				);
			}
		}

		if let Some(envs) = self.cache_envs {
			if !envs.is_empty() {
				warn!("--cache-envs is unimplemented in rtk and has no effect");
			}
		}

		if let Some(Some(_)) = self.cache_path {
			warn!("--cache-path is unimplemented in rtk and has no effect");
		}

		if let Some(Some(_)) = self.mem_ballast_size_bytes {
			warn!("--mem-ballast-size-bytes is unimplemented in rtk and has no effect");
		}
	}
}

/// A writer wrapper that silently handles broken pipe errors.
///
/// When the underlying writer returns a broken pipe error (EPIPE), this wrapper
/// converts it to a successful write. This allows commands to exit cleanly when
/// output is piped to a process that closes early (e.g., `rtk eval . | head -1`).
pub struct BrokenPipeGuard<W> {
	inner: W,
}

impl<W> BrokenPipeGuard<W> {
	pub fn new(inner: W) -> Self {
		Self { inner }
	}
}

impl<W: Write> Write for BrokenPipeGuard<W> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		match self.inner.write(buf) {
			Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(buf.len()),
			other => other,
		}
	}

	fn flush(&mut self) -> io::Result<()> {
		match self.inner.flush() {
			Err(e) if e.kind() == ErrorKind::BrokenPipe => Ok(()),
			other => other,
		}
	}
}
