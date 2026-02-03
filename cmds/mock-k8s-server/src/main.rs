//! Standalone mock Kubernetes API server for testing and benchmarking.
//!
//! This binary starts an HTTP server that simulates a Kubernetes API,
//! useful for benchmarking tools like rtk and tk without a real cluster.

use std::{
	fs, io,
	os::fd::OwnedFd,
	path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use clap::Parser;
use k8s_mock::{DiscoveryMode, HttpMockK8sServer};
use nix::unistd::{fork, pipe, setsid, write, ForkResult};
use tracing::{debug, info};

#[derive(Parser)]
#[command(name = "mock-k8s-server")]
#[command(about = "Standalone mock Kubernetes API server for testing")]
struct Cli {
	/// Directory containing YAML manifests to serve as cluster state
	#[arg(short = 'd', long, default_value = ".")]
	data_dir: PathBuf,

	/// Path to write kubeconfig file
	#[arg(short = 'k', long)]
	kubeconfig: PathBuf,

	/// Path to write PID file (optional)
	#[arg(short = 'p', long)]
	pidfile: Option<PathBuf>,

	/// Context name to use in generated kubeconfig
	#[arg(short = 'c', long, default_value = "mock-context")]
	context_name: String,

	/// Run in foreground instead of daemonizing
	#[arg(short = 'f', long)]
	foreground: bool,

	/// Use legacy discovery mode (no aggregated discovery)
	#[arg(long)]
	legacy_discovery: bool,
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	if cli.foreground {
		run_server(cli, None)
	} else {
		daemonize_and_run(cli)
	}
}

/// Daemonize, then run the server. Parent blocks until server is ready.
fn daemonize_and_run(cli: Cli) -> Result<()> {
	// Create pipe for readiness signaling
	let (read_fd, write_fd) = pipe().context("failed to create pipe")?;

	match unsafe { fork() }.context("first fork failed")? {
		ForkResult::Parent { .. } => {
			// Parent: close write end and wait for ready signal
			drop(write_fd);

			// Block reading from pipe - child writes when ready
			let read_file: std::fs::File = read_fd.into();
			let mut buf = [0u8; 1];
			std::io::Read::read_exact(&mut std::io::BufReader::new(read_file), &mut buf)
				.context("daemon failed to start")?;

			Ok(())
		}
		ForkResult::Child => {
			// Child: close read end
			drop(read_fd);

			// Create new session
			setsid().context("setsid failed")?;

			// Second fork to ensure we're not session leader
			match unsafe { fork() }.context("second fork failed")? {
				ForkResult::Parent { .. } => {
					std::process::exit(0);
				}
				ForkResult::Child => {
					// Grandchild: run the server
					run_server(cli, Some(write_fd))
				}
			}
		}
	}
}

/// Run the mock server.
fn run_server(cli: Cli, ready_fd: Option<OwnedFd>) -> Result<()> {
	// Initialize tracing
	tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::from_default_env()
				.add_directive("k8s_mock=debug".parse().unwrap())
				.add_directive("mock_k8s_server=debug".parse().unwrap()),
		)
		.with_writer(io::stderr)
		.init();

	// Build and run tokio runtime
	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.context("failed to build tokio runtime")?
		.block_on(async_main(cli, ready_fd))
}

async fn async_main(cli: Cli, ready_fd: Option<OwnedFd>) -> Result<()> {
	// Load manifests from data directory
	let manifests = load_manifests_from_dir(&cli.data_dir)?;
	info!(count = manifests.len(), dir = %cli.data_dir.display(), "Loaded manifests");

	// Determine discovery mode
	let discovery_mode = if cli.legacy_discovery {
		DiscoveryMode::Legacy
	} else {
		DiscoveryMode::Aggregated
	};

	// Start the mock server
	let server = HttpMockK8sServer::builder()
		.discovery_mode(discovery_mode)
		.resources(manifests)
		.build()
		.start()
		.await;

	let uri = server.uri();
	info!(uri = %uri, "Mock Kubernetes server started");

	// Generate and write kubeconfig
	let kubeconfig = server.kubeconfig_with_context(&cli.context_name);
	let kubeconfig_yaml =
		serde_yaml::to_string(&kubeconfig).context("failed to serialize kubeconfig")?;
	fs::write(&cli.kubeconfig, &kubeconfig_yaml)
		.with_context(|| format!("failed to write kubeconfig to {}", cli.kubeconfig.display()))?;
	info!(path = %cli.kubeconfig.display(), "Wrote kubeconfig");

	// Write PID file
	if let Some(pidfile) = &cli.pidfile {
		fs::write(pidfile, format!("{}\n", std::process::id()))
			.with_context(|| format!("failed to write pidfile to {}", pidfile.display()))?;
		debug!(path = %pidfile.display(), "Wrote PID file");
	}

	// Signal readiness to parent (if daemonized)
	if let Some(fd) = ready_fd {
		write(&fd, b"R").ok();
		drop(fd);
	}

	// Wait for shutdown signal (SIGINT or SIGTERM)
	info!("Waiting for shutdown signal");
	let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
		.context("failed to register SIGTERM handler")?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => {
			info!("Received SIGINT");
		}
		_ = sigterm.recv() => {
			info!("Received SIGTERM");
		}
	}

	info!("Shutting down");

	// Clean up PID file
	if let Some(pidfile) = &cli.pidfile {
		let _ = fs::remove_file(pidfile);
	}

	Ok(())
}

/// Load YAML manifests from a directory.
fn load_manifests_from_dir(dir: &Path) -> Result<Vec<serde_json::Value>> {
	use serde::Deserialize;

	let mut manifests = Vec::new();

	if !dir.exists() {
		return Ok(manifests);
	}

	let mut entries: Vec<_> = fs::read_dir(dir)
		.with_context(|| format!("failed to read directory {}", dir.display()))?
		.filter_map(|e| e.ok())
		.filter(|e| {
			e.path()
				.extension()
				.map(|ext| ext == "yaml" || ext == "yml")
				.unwrap_or(false)
		})
		.collect();
	entries.sort_by_key(|e| e.path());

	for entry in entries {
		let path = entry.path();
		let content = fs::read_to_string(&path)
			.with_context(|| format!("failed to read {}", path.display()))?;

		// Handle multi-document YAML files
		for doc in serde_yaml::Deserializer::from_str(&content) {
			let value = serde_json::Value::deserialize(doc)
				.with_context(|| format!("failed to parse YAML in {}", path.display()))?;

			// Skip empty documents
			if value.is_null() {
				continue;
			}

			debug!(path = %path.display(), kind = ?value.get("kind"), "Loaded manifest");
			manifests.push(value);
		}
	}

	Ok(manifests)
}
