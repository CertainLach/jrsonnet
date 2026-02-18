//! Charts subcommand handler. Mirrors tk tool charts (init, add, add-repo, vendor, config, version-check).

use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};

use super::chartfile::{
	self as cf, default_chartfile, is_valid_repo_name, load_chartfile, write_chartfile, Chartfile,
	RequiresVersionInfo,
};
use super::helm_exec::{self as helm, get_repositories, read_chart_version};

#[derive(Args)]
pub struct ChartsArgs {
	#[command(subcommand)]
	pub command: ChartsCommands,
}

#[derive(Subcommand)]
pub enum ChartsCommands {
	/// Create a new Chartfile
	Init(ChartsInitArgs),

	/// Adds Charts to the chartfile
	Add(ChartsAddArgs),

	/// Adds a repository to the chartfile
	AddRepo(ChartsAddRepoArgs),

	/// Download Charts to a local folder
	Vendor(ChartsVendorArgs),

	/// Displays the current manifest
	Config(ChartsConfigArgs),

	/// Check required charts for updated versions
	VersionCheck(ChartsVersionCheckArgs),
}

#[derive(Args)]
pub struct ChartsInitArgs {}

#[derive(Args)]
pub struct ChartsAddArgs {
	/// Charts to add (format: chart@version)
	pub charts: Vec<String>,

	/// Specify a local helm repository config file to use instead of the repositories in the chartfile.yaml. For use with private repositories
	#[arg(long)]
	pub repository_config: Option<String>,
}

#[derive(Args)]
pub struct ChartsAddRepoArgs {
	/// Repository name
	pub name: String,

	/// Repository URL
	pub url: String,
}

#[derive(Args)]
pub struct ChartsVendorArgs {
	/// Also remove non-vendored files from the destination directory
	#[arg(long)]
	pub prune: bool,

	/// Specify a local helm repository config file to use instead of the repositories in the chartfile.yaml. For use with private repositories
	#[arg(long)]
	pub repository_config: Option<String>,
}

#[derive(Args)]
pub struct ChartsConfigArgs {}

#[derive(Args)]
pub struct ChartsVersionCheckArgs {
	/// Pretty print json output with indents
	#[arg(long)]
	pub pretty_print: bool,

	/// Specify a local helm repository config file to use instead of the repositories in the chartfile.yaml. For use with private repositories
	#[arg(long)]
	pub repository_config: Option<String>,
}

/// Run the charts subcommand.
pub fn run<W: Write>(args: ChartsArgs, writer: W) -> Result<()> {
	match args.command {
		ChartsCommands::Init(init_args) => run_init(init_args, writer),
		ChartsCommands::Add(add_args) => run_add(add_args, writer),
		ChartsCommands::AddRepo(add_repo_args) => run_add_repo(add_repo_args, writer),
		ChartsCommands::Vendor(vendor_args) => run_vendor(vendor_args, writer),
		ChartsCommands::Config(config_args) => run_config(config_args, writer),
		ChartsCommands::VersionCheck(version_check_args) => {
			run_version_check(version_check_args, writer)
		}
	}
}

fn run_init<W: Write>(_args: ChartsInitArgs, _writer: W) -> Result<()> {
	let cwd = std::env::current_dir().map_err(|e| anyhow!("current dir: {}", e))?;
	let path = cwd.join(cf::FILENAME);
	if path.exists() {
		return Err(anyhow!(
			"chartfile at '{}' already exists. Aborting",
			path.display()
		));
	}
	let c = default_chartfile();
	write_chartfile(&c, &path)?;
	eprintln!("Success! New Chartfile created at '{}'", path.display());
	Ok(())
}

fn run_config<W: Write>(_args: ChartsConfigArgs, mut writer: W) -> Result<()> {
	let cwd = std::env::current_dir().map_err(|e| anyhow!("current dir: {}", e))?;
	let c = load_chartfile(&cwd)?;
	let data = serde_yaml_with_quirks::to_string(&c).map_err(|e| anyhow!("serialize: {}", e))?;
	writer.write_all(data.as_bytes())?;
	Ok(())
}

fn run_add_repo<W: Write>(args: ChartsAddRepoArgs, _writer: W) -> Result<()> {
	let cwd = std::env::current_dir().map_err(|e| anyhow!("current dir: {}", e))?;
	let path = cwd.join(cf::FILENAME);
	let mut c = load_chartfile(&cwd)?;

	let new_repo = cf::Repo {
		name: args.name.clone(),
		url: args.url,
		ca_file: String::new(),
		cert_file: String::new(),
		key_file: String::new(),
		username: String::new(),
		password: String::new(),
	};

	if cf::repos_has(&c.repositories, &new_repo) {
		eprintln!("Skipping {}. already exists", args.name);
		return Err(anyhow!(
			"1 Repo(s) were skipped. Please check above logs for details"
		));
	}
	if !is_valid_repo_name(&args.name) {
		eprintln!(
			"Skipping {}. invalid name. cannot contain any special characters",
			args.name
		);
		return Err(anyhow!(
			"1 Repo(s) were skipped. Please check above logs for details"
		));
	}
	c.repositories.push(new_repo);
	write_chartfile(&c, &path)?;
	Ok(())
}

fn run_add<W: Write>(args: ChartsAddArgs, _writer: W) -> Result<()> {
	let cwd = std::env::current_dir().map_err(|e| anyhow!("current dir: {}", e))?;
	let path = cwd.join(cf::FILENAME);
	let mut c = load_chartfile(&cwd)?;
	let repo_config = args.repository_config.as_deref().map(Path::new);

	let mut added = 0usize;

	for s in &args.charts {
		let req = match cf::parse_req(s) {
			Ok(r) => r,
			Err(e) => {
				eprintln!("Skipping {}. {}", s, e);
				continue;
			}
		};
		if cf::requirements_has(&c.requires, &req) {
			eprintln!("Skipping {}. already exists", s);
			continue;
		}
		c.requires.push(req);
		added += 1;
		eprintln!("OK: {}", s);
	}

	cf::requirements_validate(&c.requires)?;
	write_chartfile(&c, &path)?;

	if added != args.charts.len() {
		return Err(anyhow!(
			"{} Chart(s) were skipped. Please check above logs for details",
			args.charts.len() - added
		));
	}

	eprintln!("Added {} Charts to chartfile.yaml. Vendoring ...", added);
	vendor_impl(&c, &cwd, false, repo_config)
}

fn run_vendor<W: Write>(args: ChartsVendorArgs, _writer: W) -> Result<()> {
	let cwd = std::env::current_dir().map_err(|e| anyhow!("current dir: {}", e))?;
	let c = load_chartfile(&cwd)?;
	let repo_config = args.repository_config.as_deref().map(Path::new);
	vendor_impl(&c, &cwd, args.prune, repo_config)
}

fn vendor_impl(
	manifest: &Chartfile,
	project_root: &Path,
	prune: bool,
	repo_config_path: Option<&Path>,
) -> Result<()> {
	let dir = project_root.join(if manifest.directory.is_empty() {
		cf::DEFAULT_DIR
	} else {
		&manifest.directory
	});
	std::fs::create_dir_all(&dir).map_err(|e| anyhow!("create charts dir: {}", e))?;

	let repositories = get_repositories(manifest, repo_config_path)?;
	cf::requirements_validate(&manifest.requires)?;

	let mut expected_dirs = std::collections::HashSet::new();
	let mut repositories_updated = false;

	for r in &manifest.requires {
		let chart_sub_dir = if r.directory.is_empty() {
			cf::parse_req_name(&r.chart).to_string()
		} else {
			r.directory.clone()
		};
		expected_dirs.insert(chart_sub_dir.clone());
		let chart_path = dir.join(&chart_sub_dir);
		let chart_manifest_path = chart_path.join("Chart.yaml");

		let chart_dir_exists = chart_path.exists();
		let chart_manifest_exists = chart_manifest_path.exists();

		if chart_manifest_exists {
			if let Ok(installed_version) = read_chart_version(&chart_path) {
				if installed_version == r.version {
					continue;
				}
			}
		}
		if chart_dir_exists {
			let _ = std::fs::remove_dir_all(&chart_path);
		}

		if !repositories_updated {
			helm::helm_repo_update(&repositories)?;
			repositories_updated = true;
		}

		let repo_name = cf::parse_req_repo(&r.chart);
		if !cf::repos_has_name(&repositories, repo_name) {
			return Err(anyhow!(
				"repository \"{}\" not found for chart \"{}\"",
				repo_name,
				r.chart
			));
		}

		helm::helm_pull(&r.chart, &r.version, &repositories, &dir, &r.directory)?;
	}

	if prune {
		let entries = std::fs::read_dir(&dir).map_err(|e| anyhow!("list charts dir: {}", e))?;
		for e in entries {
			let e = e?;
			let name = e
				.file_name()
				.into_string()
				.map_err(|_| anyhow!("non-utf8 name"))?;
			if !expected_dirs.contains(&name) {
				let is_dir = e.file_type()?.is_dir();
				let item_type = if is_dir { "directory" } else { "file" };
				eprintln!("Pruning {}: {}", item_type, name);
				let p = dir.join(&name);
				if is_dir {
					std::fs::remove_dir_all(&p)?;
				} else {
					std::fs::remove_file(&p)?;
				}
			}
		}
	}

	Ok(())
}

fn run_version_check<W: Write>(args: ChartsVersionCheckArgs, writer: W) -> Result<()> {
	let cwd = std::env::current_dir().map_err(|e| anyhow!("current dir: {}", e))?;
	let c = load_chartfile(&cwd)?;
	let repo_config = args.repository_config.as_deref().map(Path::new);
	let repositories = get_repositories(&c, repo_config)?;

	let mut out: std::collections::HashMap<String, RequiresVersionInfo> =
		std::collections::HashMap::new();

	for r in &c.requires {
		let search_versions = helm::helm_search_repo(&r.chart, &r.version, &repositories)?;
		let latest = search_versions
			.first()
			.cloned()
			.unwrap_or(cf::ChartSearchVersion {
				name: Some(r.chart.clone()),
				version: Some(r.version.clone()),
				app_version: None,
				description: None,
			});
		let using_latest = latest.version.as_deref() == Some(r.version.as_str());
		let key = format!("{}@{}", r.chart, r.version);
		out.insert(
			key,
			RequiresVersionInfo {
				name: Some(r.chart.clone()),
				directory: if r.directory.is_empty() {
					None
				} else {
					Some(r.directory.clone())
				},
				current_version: Some(r.version.clone()),
				using_latest_version: using_latest,
				latest_version: latest.clone(),
				latest_matching_major_version: search_versions
					.get(1)
					.cloned()
					.unwrap_or_else(|| latest.clone()),
				latest_matching_minor_version: search_versions
					.get(2)
					.cloned()
					.unwrap_or_else(|| latest.clone()),
			},
		);
	}

	if args.pretty_print {
		serde_json::to_writer_pretty(writer, &out).map_err(|e| anyhow!("json: {}", e))?;
	} else {
		serde_json::to_writer(writer, &out).map_err(|e| anyhow!("json: {}", e))?;
	}
	Ok(())
}
