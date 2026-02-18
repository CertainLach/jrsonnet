//! Simple telemetry setup for tracing and logging.

use std::io::IsTerminal;

use tracing::Level;
use tracing_subscriber::{
	fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

/// Initialize tracing with RUST_LOG support.
///
/// Output format:
/// - Pretty format if stderr is a terminal
/// - JSON format otherwise
///
/// Spans are logged on entry and exit when at DEBUG level or below.
pub fn init() {
	let filter = EnvFilter::builder()
		.with_default_directive(Level::WARN.into())
		.from_env_lossy();

	let fmt_layer = if std::io::stderr().is_terminal() {
		tracing_subscriber::fmt::layer()
			.with_writer(std::io::stderr)
			.with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
			.pretty()
			.boxed()
	} else {
		tracing_subscriber::fmt::layer()
			.with_writer(std::io::stderr)
			.with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
			.json()
			.boxed()
	};

	tracing_subscriber::registry()
		.with(filter)
		.with(fmt_layer)
		.init();
}
