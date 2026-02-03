//! Telemetry setup for tracing and logging.

use std::io::IsTerminal;

/// Environment variable for service name (not exported by opentelemetry_sdk).
const OTEL_SERVICE_NAME: &str = "OTEL_SERVICE_NAME";

use anyhow::Result;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Guard that ensures OpenTelemetry traces are flushed on drop.
pub struct TelemetryGuard {
	_inner: Option<OtelGuard>,
}

/// Guard that shuts down the tracer provider on drop.
struct OtelGuard {
	tracer_provider: SdkTracerProvider,
}

impl Drop for OtelGuard {
	fn drop(&mut self) {
		if let Err(e) = self.tracer_provider.shutdown() {
			eprintln!("Failed to shutdown tracer provider: {e}");
		}
	}
}

/// Check if OpenTelemetry export is configured via environment variables.
fn otel_export_enabled() -> bool {
	std::env::var(opentelemetry_otlp::OTEL_EXPORTER_OTLP_ENDPOINT).is_ok()
		|| std::env::var(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TRACES_ENDPOINT).is_ok()
}

/// Initialize tracing with the given log level.
///
/// Priority for log level:
/// 1. `log_level` argument (from --log-level CLI flag)
/// 2. `RUST_LOG` environment variable
/// 3. Default: info
///
/// Output format:
/// - Pretty format if stderr is a terminal
/// - JSON format otherwise
///
/// OpenTelemetry:
/// - Enabled when `OTEL_EXPORTER_OTLP_ENDPOINT` or `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` is set
/// - Configured via standard OTEL env vars (OTEL_SERVICE_NAME, OTEL_RESOURCE_ATTRIBUTES, etc.)
/// - Default service.name: "rtk" (if OTEL_SERVICE_NAME is not set)
pub fn init(log_level: Option<Level>) -> Result<TelemetryGuard> {
	// Build filter layer: CLI flag takes priority, then RUST_LOG, then default to info
	let filter_layer = match log_level {
		Some(level) => EnvFilter::new(level.as_str()),
		None => EnvFilter::builder()
			.with_default_directive(Level::INFO.into())
			.from_env_lossy(),
	};

	let is_terminal = std::io::stderr().is_terminal();

	// Build fmt layer with TTY-based format selection
	let fmt_layer = if is_terminal {
		tracing_subscriber::fmt::layer()
			.with_writer(std::io::stderr)
			.pretty()
			.boxed()
	} else {
		tracing_subscriber::fmt::layer()
			.with_writer(std::io::stderr)
			.json()
			.boxed()
	};

	// Conditionally add OpenTelemetry layer
	if otel_export_enabled() {
		let (otel_layer, guard) = init_otel()?;

		tracing_subscriber::registry()
			.with(filter_layer)
			.with(fmt_layer)
			.with(otel_layer)
			.init();

		return Ok(TelemetryGuard {
			_inner: Some(guard),
		});
	}

	tracing_subscriber::registry()
		.with(filter_layer)
		.with(fmt_layer)
		.init();

	Ok(TelemetryGuard { _inner: None })
}

fn init_otel<S>() -> Result<(impl Layer<S>, OtelGuard)>
where
	S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
	use opentelemetry_sdk::Resource;

	let mut resource_builder = Resource::builder();

	// Resource::builder() reads `OTEL_SERVICE_NAME` and `OTEL_RESOURCE_ATTRIBUTES` automatically.
	// We set "rtk" as fallback only if `OTEL_SERVICE_NAME` is not set.
	if std::env::var(OTEL_SERVICE_NAME).is_err() {
		resource_builder = resource_builder.with_service_name("rtk");
	};

	let resource = resource_builder.build();

	// Build OTLP exporter - select transport based on `OTEL_EXPORTER_OTLP_PROTOCOL`
	let exporter = match std::env::var(opentelemetry_otlp::OTEL_EXPORTER_OTLP_PROTOCOL)
		.as_deref()
		.unwrap_or(opentelemetry_otlp::OTEL_EXPORTER_OTLP_PROTOCOL_DEFAULT)
	{
		"grpc" => opentelemetry_otlp::SpanExporter::builder()
			.with_tonic()
			.build()?,
		_ => opentelemetry_otlp::SpanExporter::builder()
			.with_http()
			.build()?,
	};

	let tracer_provider = SdkTracerProvider::builder()
		.with_resource(resource)
		.with_batch_exporter(exporter)
		.build();

	let layer = tracing_opentelemetry::layer()
		.with_error_records_to_exceptions(true)
		.with_tracer(tracer_provider.tracer("rtk"));

	// Set global tracer provider
	opentelemetry::global::set_tracer_provider(tracer_provider.clone());

	Ok((layer, OtelGuard { tracer_provider }))
}
