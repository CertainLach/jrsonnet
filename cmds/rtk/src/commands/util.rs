//! Utilities for command handlers.

use std::io::{self, ErrorKind, Write};

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
