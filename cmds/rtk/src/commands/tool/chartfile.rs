//! Chartfile types and validation (chartfile.yaml manifest).
//! Mirrors github.com/grafana/tanka/pkg/helm spec and charts behavior.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const VERSION: u32 = 1;
pub const FILENAME: &str = "chartfile.yaml";
pub const DEFAULT_DIR: &str = "charts";

/// Chartfile is the schema used to declaratively define locally required Helm Charts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chartfile {
	/// Version of the Chartfile schema (for future use)
	pub version: u32,

	/// Repositories to source from
	pub repositories: Repos,

	/// Requires lists Charts expected to be present in the charts folder
	pub requires: Requirements,

	/// Folder to use for storing Charts. Defaults to 'charts'
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub directory: String,
}

/// Helm repository config file (repositories.yaml) used in place of chartfile repositories when supplied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
	pub api_version: Option<String>,
	pub generated: Option<String>,
	pub repositories: Repos,
}

/// A single Helm repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub name: String,
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub url: String,
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub ca_file: String,
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub cert_file: String,
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub key_file: String,
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub username: String,
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub password: String,
}

pub type Repos = Vec<Repo>;

/// A single required Helm Chart. Both chart and version are required.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
	pub chart: String,
	pub version: String,
	#[serde(skip_serializing_if = "String::is_empty", default)]
	pub directory: String,
}

pub type Requirements = Vec<Requirement>;

/// Chart version info from `helm search repo`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ChartSearchVersion {
	pub name: Option<String>,
	pub version: Option<String>,
	#[serde(rename = "app_version")]
	pub app_version: Option<String>,
	pub description: Option<String>,
}

/// Version check result for one required chart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct RequiresVersionInfo {
	pub name: Option<String>,
	pub directory: Option<String>,
	pub current_version: Option<String>,
	pub using_latest_version: bool,
	pub latest_version: ChartSearchVersion,
	pub latest_matching_major_version: ChartSearchVersion,
	pub latest_matching_minor_version: ChartSearchVersion,
}

/// Returns true if repos contains repo (by value equality).
pub fn repos_has(repos: &[Repo], repo: &Repo) -> bool {
	repos.iter().any(|r| r == repo)
}

/// Returns true if any repo has the given name.
pub fn repos_has_name(repos: &[Repo], name: &str) -> bool {
	repos.iter().any(|r| r.name == name)
}

/// Returns true if requirements contains req (by value equality).
pub fn requirements_has(requirements: &[Requirement], req: &Requirement) -> bool {
	requirements.iter().any(|r| r == req)
}

/// Validates requirements: repo/name format and unique output directories.
pub fn requirements_validate(requirements: &[Requirement]) -> Result<()> {
	let mut output_dirs: std::collections::HashMap<String, &Requirement> =
		std::collections::HashMap::new();
	let mut errs = Vec::new();

	for req in requirements {
		if !req.chart.contains('/') {
			errs.push(format!(
				"Chart name \"{}\" is not valid. Expecting a repo/name format.",
				req.chart
			));
			continue;
		}
		let dir = if req.directory.is_empty() {
			parse_req_name(&req.chart).to_string()
		} else {
			req.directory.clone()
		};
		if let Some(previous) = output_dirs.get(&dir) {
			errs.push(format!(
				"output directory \"{}\" is used twice, by charts \"{}@{}\" and \"{}@{}\"",
				dir, previous.chart, previous.version, req.chart, req.version
			));
		} else {
			output_dirs.insert(dir, req);
		}
	}

	if errs.is_empty() {
		Ok(())
	} else {
		Err(anyhow!("validation errors:\n - {}", errs.join("\n - ")))
	}
}

/// Parses a requirement from a string of the format `repo/name@version` or `repo/name@version:path`.
/// Regex from tanka: `^(?P<chart>[\w+-\/.]+)@(?P<version>[^:\n\s]+)(?:\:(?P<directory>[\w-. ]+))?$`
pub fn parse_req(s: &str) -> Result<Requirement> {
	// Chart: one or more [\w+-\/.], then @, then version [^:\n\s]+, optional :directory [\w-. ]+
	let re =
		regex::Regex::new(r"^([\w+\-/.]+)@([^:\n\s]+)(?::([\w\-\. ]+))?$").expect("chart regex");
	let cap = re.captures(s).ok_or_else(|| {
		anyhow!("not of form 'repo/chart@version(:path)' where repo contains no special characters")
	})?;
	let chart = cap.get(1).map(|m| m.as_str()).unwrap_or("");
	let version = cap.get(2).map(|m| m.as_str()).unwrap_or("");
	let directory = cap.get(3).map(|m| m.as_str()).unwrap_or("").to_string();

	// Repo part (before first /) must match ^[\w-]+$ - no special chars
	let repo_part = parse_req_repo(chart);
	let repo_re = regex::Regex::new(r"^[\w-]+$").expect("repo name regex");
	if !repo_re.is_match(repo_part) {
		return Err(anyhow!(
			"not of form 'repo/chart@version(:path)' where repo contains no special characters"
		));
	}

	Ok(Requirement {
		chart: chart.to_string(),
		version: version.to_string(),
		directory,
	})
}

/// Repo name from `repo/name` (element before first /).
pub fn parse_req_repo(chart: &str) -> &str {
	chart.split_once('/').map(|(r, _)| r).unwrap_or(chart)
}

/// Chart name from `repo/name` (element after first /).
pub fn parse_req_name(chart: &str) -> &str {
	chart.split_once('/').map(|(_, n)| n).unwrap_or("")
}

/// Load Chartfile from project root directory.
pub fn load_chartfile(project_root: &Path) -> Result<Chartfile> {
	let path = project_root.join(FILENAME);
	let data =
		std::fs::read_to_string(&path).map_err(|e| anyhow!("failed to read chartfile: {}", e))?;
	let mut c: Chartfile = serde_yaml_with_quirks::from_str(&data)
		.map_err(|e| anyhow!("failed to parse chartfile: {}", e))?;
	for (i, r) in c.requires.iter().enumerate() {
		if r.chart.is_empty() {
			return Err(anyhow!("requirements[{}]: 'chart' must be set", i));
		}
	}
	if c.directory.is_empty() {
		c.directory = DEFAULT_DIR.to_string();
	}
	Ok(c)
}

/// Write Chartfile to path.
pub fn write_chartfile(c: &Chartfile, path: &Path) -> Result<()> {
	let data = serde_yaml_with_quirks::to_string(c)
		.map_err(|e| anyhow!("failed to serialize chartfile: {}", e))?;
	std::fs::write(path, data).map_err(|e| anyhow!("failed to write chartfile: {}", e))?;
	Ok(())
}

/// Create initial Chartfile content (for init).
pub fn default_chartfile() -> Chartfile {
	Chartfile {
		version: VERSION,
		repositories: vec![Repo {
			name: "stable".to_string(),
			url: "https://charts.helm.sh/stable".to_string(),
			ca_file: String::new(),
			cert_file: String::new(),
			key_file: String::new(),
			username: String::new(),
			password: String::new(),
		}],
		requires: Vec::new(),
		directory: DEFAULT_DIR.to_string(),
	}
}

/// Load Helm repo config from file (for --repository-config).
pub fn load_helm_repo_config(path: &Path) -> Result<ConfigFile> {
	let data = std::fs::read_to_string(path)
		.map_err(|e| anyhow!("failed to read repository config: {}", e))?;
	serde_yaml_with_quirks::from_str(&data)
		.map_err(|e| anyhow!("failed to parse repository config: {}", e))
}

/// Repo name validation: only \w- allowed (tanka: repoExp = `^[\w-]+$`).
pub fn is_valid_repo_name(name: &str) -> bool {
	let re = regex::Regex::new(r"^[\w-]+$").expect("repo name regex");
	re.is_match(name)
}
