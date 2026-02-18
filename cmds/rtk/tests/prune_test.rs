//! Integration tests for the prune command using a mock Kubernetes API server.
//!
//! These tests focus on prune-command behavior (dry-run, confirmation, deletion),
//! while prune detection correctness is primarily covered by diff tests with
//! `with_prune=true`.

use std::os::fd::FromRawFd;
use std::path::{Path, PathBuf};

#[path = "test_utils.rs"]
mod test_utils;

use k8s_mock::DiscoveryMode;
use rtk::{
	commands::{
		diff::ColorMode,
		prune::{prune_environment, AutoApprove, PruneOpts},
	},
	k8s::{client::ClusterConnection, diff::ResourceDiff},
};

fn fixture_dir(name: &str) -> PathBuf {
	test_utils::diff_fixture_dir(name)
}

async fn setup_connection(
	test_dir: &Path,
	discovery_mode: DiscoveryMode,
) -> (k8s_mock::RunningHttpMockK8sServer, ClusterConnection) {
	let cluster_state = test_utils::load_manifests_from_dir(&test_dir.join("cluster"));
	test_utils::setup_connection_from_cluster_state(cluster_state, discovery_mode, false).await
}

async fn run_prune(
	env_dir: &Path,
	connection: ClusterConnection,
	opts: PruneOpts,
) -> anyhow::Result<Vec<ResourceDiff>> {
	let mut output = Vec::new();
	prune_environment(
		env_dir.to_str().expect("env path should be UTF-8"),
		Some(connection),
		rtk::eval::EvalOpts::default(),
		opts,
		&mut output,
	)
	.await
}

fn default_prune_opts() -> PruneOpts {
	PruneOpts {
		auto_approve: AutoApprove::Always,
		color: ColorMode::Never,
		..Default::default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn prune_dry_run_does_not_delete() {
		let test_dir = fixture_dir("resource_deleted");
		let env_dir = test_dir.join("environment");
		let (_server, connection) = setup_connection(&test_dir, DiscoveryMode::Aggregated).await;

		let mut dry_run_opts = default_prune_opts();
		dry_run_opts.dry_run = Some("client".to_string());

		let dry_run_diffs = run_prune(&env_dir, connection.clone(), dry_run_opts)
			.await
			.expect("dry-run prune should succeed");
		assert_eq!(dry_run_diffs.len(), 1, "dry-run should find one deletion");
		assert_eq!(dry_run_diffs[0].name, "delete-this");

		let delete_diffs = run_prune(&env_dir, connection, default_prune_opts())
			.await
			.expect("real prune after dry-run should succeed");
		assert_eq!(
			delete_diffs.len(),
			1,
			"resource should still exist after dry-run"
		);
		assert_eq!(delete_diffs[0].name, "delete-this");
	}

	#[tokio::test]
	async fn prune_is_idempotent_after_delete() {
		let test_dir = fixture_dir("resource_deleted");
		let env_dir = test_dir.join("environment");
		let (_server, connection) = setup_connection(&test_dir, DiscoveryMode::Legacy).await;

		let first = run_prune(&env_dir, connection.clone(), default_prune_opts())
			.await
			.expect("first prune should succeed");
		assert_eq!(
			first.len(),
			1,
			"first prune should delete exactly one resource"
		);
		assert_eq!(first[0].name, "delete-this");

		let second = run_prune(&env_dir, connection, default_prune_opts())
			.await
			.expect("second prune should succeed");
		assert!(
			second.is_empty(),
			"second prune should have nothing left to delete"
		);
	}

	#[tokio::test]
	async fn prune_auto_approve_never_errors_in_non_interactive_mode() {
		// Close stdin so is_terminal() returns false even when the test
		// runner is invoked from an interactive terminal.
		drop(unsafe { std::os::fd::OwnedFd::from_raw_fd(0) });

		let test_dir = fixture_dir("resource_deleted");
		let env_dir = test_dir.join("environment");
		let (_server, connection) = setup_connection(&test_dir, DiscoveryMode::Aggregated).await;

		let mut opts = default_prune_opts();
		opts.auto_approve = AutoApprove::Never;

		let err = run_prune(&env_dir, connection, opts)
			.await
			.expect_err("non-interactive prune should fail when prompting is required");
		let msg = format!("{err:#}");
		assert!(
			msg.contains("cannot prompt for confirmation in non-interactive mode"),
			"unexpected error message: {msg}"
		);
	}

	#[tokio::test]
	async fn prune_implicit_namespace_no_false_deletion() {
		let test_dir = fixture_dir("implicit_namespace_prune");
		let env_dir = test_dir.join("environment");
		let (_server, connection) = setup_connection(&test_dir, DiscoveryMode::Aggregated).await;

		let diffs = run_prune(&env_dir, connection, default_prune_opts())
			.await
			.expect("prune should succeed");
		assert!(
			diffs.is_empty(),
			"expected no deletions, but found: {:?}",
			diffs
				.iter()
				.map(|d| format!("{}/{}", d.gvk.kind, d.name))
				.collect::<Vec<_>>()
		);
	}
}
