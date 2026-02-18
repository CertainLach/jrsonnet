//! Helm CLI execution (Pull, RepoUpdate, SearchRepo). Mirrors tanka ExecHelm.

use crate::commands::tool::chartfile::{ChartSearchVersion, Chartfile, Repo, Repos};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

fn helm_bin() -> String {
	std::env::var("RTK_HELM_PATH").unwrap_or_else(|_| "helm".to_string())
}

/// Write repos to a temp file for --repository-config. Returns path.
fn write_repo_tmp_file(repos: &[Repo]) -> Result<std::sync::Arc<tempfile::TempPath>> {
	let mut m = serde_json::Map::new();
	m.insert(
		"repositories".to_string(),
		serde_json::to_value(repos).map_err(|e| anyhow!("serialize repos: {}", e))?,
	);
	let json = serde_json::to_string(&m).map_err(|e| anyhow!("serialize: {}", e))?;
	// Helm expects YAML for repository-config; tanka writes JSON to temp and passes to helm.
	// Actually tanka uses json.NewEncoder and writes JSON. Let me check helm docs - helm repo
	// update --repository-config accepts a file. The format is the same as ~/.config/helm/repositories.yaml
	// which is YAML with "repositories:" key. So we need to write YAML. Tanka writes JSON - maybe helm
	// accepts both? Checking tanka again: writeRepoTmpFile uses enc.Encode(m) which is JSON. So helm
	// accepts JSON for repository-config. We'll use JSON like tanka.
	let tmp = tempfile::NamedTempFile::new().map_err(|e| anyhow!("temp file: {}", e))?;
	std::fs::write(tmp.path(), json).map_err(|e| anyhow!("write repos: {}", e))?;
	Ok(std::sync::Arc::new(tmp.into_temp_path()))
}

/// Run helm pull, then move extracted chart to destination/extract_directory.
pub fn helm_pull(
	chart: &str,
	version: &str,
	repos: &[Repo],
	destination: &Path,
	extract_directory: &str,
) -> Result<()> {
	let repo_file = write_repo_tmp_file(repos)?;
	// Create temp dir inside destination to avoid cross-device rename issues
	let temp_dir =
		tempfile::tempdir_in(destination).map_err(|e| anyhow!("tempdir in destination: {}", e))?;
	let temp_path = temp_dir.path();

	let chart_name = crate::commands::tool::chartfile::parse_req_name(chart);
	let pull_path = chart_pull_path(chart, repos);

	let status = Command::new(helm_bin())
		.args([
			"pull",
			pull_path.as_str(),
			"--version",
			version,
			"--repository-config",
			repo_file.to_str().unwrap(),
			"--destination",
			temp_path.to_str().unwrap(),
			"--untar",
		])
		.status()
		.map_err(|e| anyhow!("helm pull: {}", e))?;
	if !status.success() {
		return Err(anyhow!("helm pull exited with {}", status));
	}

	let extract_dir = if extract_directory.is_empty() {
		chart_name
	} else {
		extract_directory
	};
	let from = temp_path.join(chart_name);
	let to = destination.join(extract_dir);
	if from != to {
		std::fs::rename(&from, &to)
			.map_err(|e| anyhow!("rename {} to {}: {}", from.display(), to.display(), e))?;
	}
	Ok(())
}

/// For OCI, chart pull path is oci://url/name; for normal repo it's repo/name.
fn chart_pull_path(chart: &str, repos: &[Repo]) -> String {
	let repo_name = crate::commands::tool::chartfile::parse_req_repo(chart);
	let chart_name = crate::commands::tool::chartfile::parse_req_name(chart);
	for r in repos {
		if r.name == repo_name && r.url.starts_with("oci://") {
			return format!("{}/{}", r.url.trim_end_matches('/'), chart_name);
		}
	}
	chart.to_string()
}

/// Run helm repo update.
pub fn helm_repo_update(repos: &[Repo]) -> Result<()> {
	let repo_file = write_repo_tmp_file(repos)?;
	let out = Command::new(helm_bin())
		.args([
			"repo",
			"update",
			"--repository-config",
			repo_file.to_str().unwrap(),
		])
		.output()
		.map_err(|e| anyhow!("helm repo update: {}", e))?;
	if !out.status.success() {
		let stderr = String::from_utf8_lossy(&out.stderr);
		return Err(anyhow!("{}\n{}", stderr, out.status));
	}
	Ok(())
}

/// Run helm search repo for one chart with version constraint; returns one ChartSearchVersion (or placeholder).
fn helm_search_one(chart: &str, version_regex: &str, repos: &[Repo]) -> Result<ChartSearchVersion> {
	let repo_file = write_repo_tmp_file(repos)?;
	// Vertical tab delimits chart name in table; tanka uses \v chart \v
	let regexp = format!("\u{0b}{}\u{0b}", chart);
	let out = Command::new(helm_bin())
		.args([
			"search",
			"repo",
			"--repository-config",
			repo_file.to_str().unwrap(),
			"--regexp",
			&regexp,
			"--version",
			version_regex,
			"-o",
			"json",
		])
		.output()
		.map_err(|e| anyhow!("helm search: {}", e))?;
	if !out.status.success() {
		let stderr = String::from_utf8_lossy(&out.stderr);
		return Err(anyhow!("{}\n{}", stderr, out.status));
	}
	let versions: Vec<ChartSearchVersion> = serde_json::from_slice(&out.stdout)
		.map_err(|e| anyhow!("parse helm search output: {}", e))?;
	if versions.len() == 1 {
		Ok(versions.into_iter().next().unwrap())
	} else {
		Ok(ChartSearchVersion {
			name: Some(chart.to_string()),
			version: None,
			app_version: None,
			description: Some("search did not return 1 version".to_string()),
		})
	}
}

/// Search for latest, latest matching major, latest matching minor (tanka order).
pub fn helm_search_repo(
	chart: &str,
	curr_version: &str,
	repos: &[Repo],
) -> Result<Vec<ChartSearchVersion>> {
	// Update repos first
	helm_repo_update(repos)?;

	let search_versions = [
		format!(">={}", curr_version), // latest
		format!("^{}", curr_version),  // latest major
		format!("~{}", curr_version),  // latest minor
	];
	let mut result = Vec::with_capacity(3);
	for version_regex in &search_versions {
		let v = helm_search_one(chart, version_regex, repos)?;
		result.push(v);
	}
	Ok(result)
}

/// Read Chart.yaml version from a vendored chart directory.
pub fn read_chart_version(chart_path: &Path) -> Result<String> {
	#[derive(Deserialize)]
	struct ChartYaml {
		version: String,
	}
	let data = std::fs::read_to_string(chart_path.join("Chart.yaml"))
		.map_err(|e| anyhow!("read Chart.yaml: {}", e))?;
	let c: ChartYaml =
		serde_yaml_with_quirks::from_str(&data).map_err(|e| anyhow!("parse Chart.yaml: {}", e))?;
	Ok(c.version)
}

/// Get repositories (from repo config file or manifest).
pub fn get_repositories(manifest: &Chartfile, repo_config_path: Option<&Path>) -> Result<Repos> {
	if let Some(p) = repo_config_path {
		let cfg = crate::commands::tool::chartfile::load_helm_repo_config(p)?;
		return Ok(cfg.repositories);
	}
	Ok(manifest.repositories.clone())
}
