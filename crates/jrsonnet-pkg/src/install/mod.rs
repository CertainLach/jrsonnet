#![allow(clippy::result_large_err)]

pub mod accessor;
mod git;
mod github;
mod hash;

use std::{
	collections::{BTreeMap, HashSet},
	fs,
	path::{Path, PathBuf},
	result,
};

use camino::{Utf8Path, Utf8PathBuf};
use tracing::{info, warn};

use crate::{
	install::hash::{Exclusions, Mode, hash_dir},
	jsonnet_bundler::{Dependency, GitScheme, GitSource, JsonnetFile, Source, SubDir},
};

pub const PKG_USER_AGENT: &str = "jrsonnet-pkg (https://delta.rocks/jrsonnet)";

pub fn cache_dir(subdir: &str) -> Result<std::path::PathBuf> {
	Ok(directories::ProjectDirs::from("rocks", "delta", "jrsonnet")
		.ok_or(Error::XdgUnavailable)?
		.cache_dir()
		.join(subdir))
}

pub(crate) struct LocalExtraction {
	/// Path inside the parent repo's tree where this local source lives.
	pub tree_path: SubDir,
	pub name: String,
}

pub(crate) struct ResolveResult {
	pub version: String,
	pub transitive_git_deps: Vec<Dependency>,
	pub local_extractions: Vec<LocalExtraction>,
	pub source: VendorSource,
}

const VERSION_FILE: &str = ".version";

/// How to populate a vendor path.
pub enum VendorSource {
	GitTree {
		repo_path: PathBuf,
		commit_sha: String,
		subdir: SubDir,
	},
	GithubZip {
		zip_path: PathBuf,
		commit_sha: String,
		subdir: SubDir,
	},
	Symlink(Utf8PathBuf),
}

impl VendorSource {
	fn with_subdir(&self, new_subdir: SubDir) -> Self {
		match self {
			VendorSource::GitTree {
				repo_path,
				commit_sha,
				..
			} => VendorSource::GitTree {
				repo_path: repo_path.clone(),
				commit_sha: commit_sha.clone(),
				subdir: new_subdir,
			},
			VendorSource::GithubZip {
				zip_path,
				commit_sha,
				..
			} => VendorSource::GithubZip {
				zip_path: zip_path.clone(),
				commit_sha: commit_sha.clone(),
				subdir: new_subdir,
			},
			VendorSource::Symlink(target) => VendorSource::Symlink(target.clone()),
		}
	}
}

pub struct InstallPlan {
	pub lock: JsonnetFile,
	/// vendor-relative path -> how to obtain it.
	pub entries: BTreeMap<Utf8PathBuf, VendorSource>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
	pub dry_run: bool,
	/// Accept checksums of trees `jsonnet-bundler` hashes incorrectly. Correct
	/// checksums are written back, so this is only needed once per lockfile.
	pub compat: bool,
}

pub fn install(
	manifest: &JsonnetFile,
	lock: Option<&JsonnetFile>,
	vendor_dir: &Path,
	opts: Options,
) -> Result<JsonnetFile, Error> {
	let mut plan = resolve(manifest, lock)?;
	execute(&plan, vendor_dir, opts)?;
	if !opts.dry_run {
		update_sums(&mut plan, vendor_dir, opts.compat)?;
	}
	Ok(plan.lock)
}

pub fn resolve(manifest: &JsonnetFile, lock: Option<&JsonnetFile>) -> Result<InstallPlan, Error> {
	let mut plan = InstallPlan {
		lock: JsonnetFile {
			version: manifest.version,
			dependencies: Vec::new(),
			legacy_imports: manifest.legacy_imports,
		},
		entries: BTreeMap::new(),
	};
	let mut installed = HashSet::new();

	resolve_deps(
		&manifest.dependencies,
		lock,
		manifest.legacy_imports,
		&mut plan,
		&mut installed,
	)?;

	Ok(plan)
}

#[cfg(unix)]
fn make_symlink(target: &str, link: &Path) -> std::io::Result<()> {
	std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn make_symlink(target: &str, link: &Path) -> std::io::Result<()> {
	std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(not(any(unix, windows)))]
fn make_symlink(_target: &str, _link: &Path) -> std::io::Result<()> {
	Err(std::io::Error::new(
		std::io::ErrorKind::Unsupported,
		"symlinks are not supported on this platform",
	))
}

fn is_up_to_date(
	plan: &InstallPlan,
	vendor_dir: &Path,
	path: &Utf8PathBuf,
	version: &str,
	compat: bool,
) -> bool {
	let dest = vendor_dir.join(path);
	if !fs::read_to_string(dest.join(VERSION_FILE)).is_ok_and(|v| v.trim() == version) {
		return false;
	}
	// The vendored tree may have been modified since it was extracted.
	let Some(expected) = locked_sum(plan, path) else {
		return true;
	};
	let exclude = exclusions(&plan.entries, vendor_dir, path);
	let matches =
		|mode| hash_dir(&dest, &exclude, Some(version), mode).is_ok_and(|sum| sum == expected);
	matches(Mode::Correct) || (compat && matches(Mode::JsonnetBundler))
}

fn write_version(dest: &Path, version: &str) -> Result<(), Error> {
	let marker = dest.join(VERSION_FILE);
	if marker.exists() {
		warn!(
			"{}: dependency ships its own {VERSION_FILE}, keeping it and re-extracting on every install",
			dest.display()
		);
		return Ok(());
	}
	fs::write(&marker, format!("{version}\n")).map_err(|e| Error::Io(marker, e))
}

pub fn execute(plan: &InstallPlan, vendor_dir: &Path, opts: Options) -> Result<(), Error> {
	let dry_run = opts.dry_run;
	if !dry_run {
		for (path, source) in &plan.entries {
			let dest = vendor_dir.join(path);
			match source {
				VendorSource::GitTree {
					repo_path,
					commit_sha,
					subdir,
				} => {
					if is_up_to_date(plan, vendor_dir, path, commit_sha, opts.compat) {
						continue;
					}
					info!("extract {path}");
					if dest.exists() {
						fs::remove_dir_all(&dest).map_err(|e| Error::Io(dest.clone(), e))?;
					}
					fs::create_dir_all(&dest).map_err(|e| Error::Io(dest.clone(), e))?;
					git::extract(repo_path, commit_sha, subdir, &dest)?;
					write_version(&dest, commit_sha)?;
				}
				VendorSource::GithubZip {
					zip_path,
					commit_sha,
					subdir,
				} => {
					if is_up_to_date(plan, vendor_dir, path, commit_sha, opts.compat) {
						continue;
					}
					info!("extract {path}");
					if dest.exists() {
						fs::remove_dir_all(&dest).map_err(|e| Error::Io(dest.clone(), e))?;
					}
					fs::create_dir_all(&dest).map_err(|e| Error::Io(dest.clone(), e))?;
					github::extract(zip_path, subdir, &dest)?;
					write_version(&dest, commit_sha)?;
				}
				VendorSource::Symlink(_) => {}
			}
		}
		for (path, source) in &plan.entries {
			if let VendorSource::Symlink(target) = source {
				let dest = vendor_dir.join(path);
				if dest
					.symlink_metadata()
					.is_ok_and(|m| m.file_type().is_symlink())
				{
					if fs::read_link(&dest).is_ok_and(|t| t == target.as_std_path()) {
						continue;
					}
					fs::remove_file(&dest).map_err(|e| Error::Io(dest.clone(), e))?;
				}
				info!("symlink {path} -> {target}");
				make_symlink(target.as_str(), &dest).map_err(|e| Error::Io(dest.clone(), e))?;
			}
		}
	}
	prune(plan, vendor_dir, dry_run)?;
	Ok(())
}

/// Nested dependencies are not part of the checksum of the dependency they are
/// nested in, as `jsonnet-bundler` wipes them when extracting the parent.
fn exclusions(
	entries: &BTreeMap<Utf8PathBuf, VendorSource>,
	vendor_dir: &Path,
	path: &Utf8Path,
) -> Exclusions {
	let prefix = format!("{path}/");
	Exclusions(
		entries
			.keys()
			.filter(|k| k.as_str().starts_with(&prefix))
			.map(|k| vendor_dir.join(k))
			.collect(),
	)
}

fn locked_sum<'a>(plan: &'a InstallPlan, path: &Utf8Path) -> Option<&'a str> {
	plan.lock
		.dependencies
		.iter()
		.find(|d| d.canonical_name() == path)?
		.sum
		.as_deref()
}

/// Verify checksums of the vendored trees against the ones carried over from
/// the lockfile, and record the missing ones.
fn update_sums(plan: &mut InstallPlan, vendor_dir: &Path, compat: bool) -> Result<(), Error> {
	let InstallPlan { lock, entries } = plan;
	for dep in &mut lock.dependencies {
		if !matches!(dep.source, Source::Git(_)) {
			continue;
		}
		let path = dep.canonical_name();
		if !entries.contains_key(&path) {
			continue;
		}
		let dir = vendor_dir.join(&path);
		let exclude = exclusions(entries, vendor_dir, &path);
		if !exclude.0.is_empty() {
			warn!(
				"{path}: nested dependencies are excluded from its checksum, jsonnet-bundler's value depends on install order here"
			);
		}
		let version = dep.version.as_deref();
		let sum = hash_dir(&dir, &exclude, version, Mode::Correct)?;
		if let Some(expected) = &dep.sum
			&& *expected != sum
		{
			// The tree itself is intact, the lockfile just carries a checksum
			// jsonnet-bundler computed incorrectly. Replace it with a correct one.
			let bundler_sum = compat
				.then(|| hash_dir(&dir, &exclude, version, Mode::JsonnetBundler))
				.transpose()?;
			if bundler_sum.as_deref() != Some(expected.as_str()) {
				return Err(Error::ChecksumMismatch {
					dep: path,
					expected: expected.clone(),
					got: sum,
				});
			}
			warn!("{path}: replacing the checksum written by jsonnet-bundler");
		}
		dep.sum = Some(sum);
	}
	Ok(())
}

fn prune(plan: &InstallPlan, vendor_dir: &Path, dry_run: bool) -> Result<(), Error> {
	if !vendor_dir.is_dir() {
		return Ok(());
	}
	prune_recursive(plan, vendor_dir, vendor_dir, dry_run)
}

fn prune_recursive(
	plan: &InstallPlan,
	vendor_dir: &Path,
	dir: &Path,
	dry_run: bool,
) -> Result<(), Error> {
	let entries = fs::read_dir(dir).map_err(|e| Error::Io(dir.to_owned(), e))?;
	for entry in entries {
		let entry = entry.map_err(|e| Error::Io(dir.to_owned(), e))?;
		let path = entry.path();
		let rel = path
			.strip_prefix(vendor_dir)
			.expect("path is under vendor_dir");
		let Ok(rel) = Utf8PathBuf::try_from(rel.to_owned()) else {
			info!("prune (non-utf8) {}", rel.display());
			continue;
		};

		if plan.entries.contains_key(&rel) {
			continue;
		}

		let ft = entry.file_type().map_err(|e| Error::Io(path.clone(), e))?;
		if ft.is_symlink() {
			info!("prune {rel}");
			if !dry_run {
				fs::remove_file(&path).map_err(|e| Error::Io(path, e))?;
			}
		} else if ft.is_dir() {
			let prefix: Utf8PathBuf = format!("{rel}/").into();
			let has_descendants = plan
				.entries
				.range(prefix.clone()..)
				.next()
				.is_some_and(|(k, _)| k.starts_with(&prefix));
			if has_descendants {
				prune_recursive(plan, vendor_dir, &path, dry_run)?;
			} else {
				info!("prune {rel}");
				if !dry_run {
					fs::remove_dir_all(&path).map_err(|e| Error::Io(path, e))?;
				}
			}
		} else {
			info!("prune {rel}");
			if !dry_run {
				fs::remove_file(&path).map_err(|e| Error::Io(path, e))?;
			}
		}
	}

	if !dry_run
		&& dir != vendor_dir
		&& let Ok(mut entries) = fs::read_dir(dir)
		&& entries.next().is_none()
	{
		let _ = fs::remove_dir(dir);
	}

	Ok(())
}

fn resolve_one(git_source: &GitSource, version: Option<&str>) -> Result<ResolveResult, Error> {
	if git_source.host == "github.com" && git_source.scheme == GitScheme::Https {
		match github::resolve(git_source, version) {
			Ok(result) => return Ok(result),
			Err(e) => {
				info!("github archive failed ({e}), falling back to git");
			}
		}
	}
	git::resolve(git_source, version)
}

fn locked_dep<'a>(dep: &Dependency, lock: Option<&'a JsonnetFile>) -> Option<&'a Dependency> {
	let key = dep.canonical_name();
	lock?
		.dependencies
		.iter()
		.find(|d| d.canonical_name() == key)
}

fn resolve_deps(
	deps: &[Dependency],
	lock: Option<&JsonnetFile>,
	legacy_imports: bool,
	plan: &mut InstallPlan,
	installed: &mut HashSet<Utf8PathBuf>,
) -> Result<(), Error> {
	for dep in deps {
		let Source::Git(git_source) = &dep.source else {
			continue;
		};

		let canonical = dep.canonical_name();
		if !installed.insert(canonical.clone()) {
			continue;
		}

		let locked = locked_dep(dep, lock);
		let version = locked
			.and_then(|d| d.version.as_deref())
			.or(dep.version.as_deref());

		info!(
			"resolving {canonical} (version: {})",
			version.unwrap_or("<TBD>")
		);

		let result = resolve_one(git_source, version)?;

		// Only the lockfile checksum is authoritative, and only for the exact
		// version it was recorded for.
		let expected_sum = locked
			.filter(|d| d.version.as_deref() == Some(result.version.as_str()))
			.and_then(|d| d.sum.clone());

		plan.lock.dependencies.push(Dependency {
			source: dep.source.clone(),
			version: Some(result.version),
			sum: expected_sum,
			name: dep.name.clone(),
			single: dep.single,
		});

		let mut repo_base = Utf8PathBuf::from(git_source.host.as_str());
		repo_base.push(git_source.plain_repo_name());

		// Legacy symlink for the dep. Skipped if `legacyImports: false`, unless
		// the user explicitly set `dep.name` (which is always honored).
		if legacy_imports || dep.name.is_some() {
			let legacy = Utf8PathBuf::from(dep.legacy_link_name());
			if legacy != canonical {
				plan.entries
					.insert(legacy, VendorSource::Symlink(canonical.clone()));
			}
		}

		for extraction in &result.local_extractions {
			let extraction_canonical = repo_base.join(&extraction.tree_path);
			plan.entries.insert(
				extraction_canonical.clone(),
				result.source.with_subdir(extraction.tree_path.clone()),
			);
			if legacy_imports {
				let extraction_name = Utf8PathBuf::from(&extraction.name);
				if extraction_name != extraction_canonical {
					plan.entries
						.insert(extraction_name, VendorSource::Symlink(extraction_canonical));
				}
			}
		}

		// Main entry (after local extractions used with_subdir)
		plan.entries.insert(canonical, result.source);

		resolve_deps(
			&result.transitive_git_deps,
			lock,
			legacy_imports,
			plan,
			installed,
		)?;
	}

	Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("io error for {0}: {1}")]
	Io(PathBuf, std::io::Error),
	#[error("failed to discover xdg directories")]
	XdgUnavailable,
	#[error("git clone failed: {0}")]
	GitClone(#[from] gix::clone::Error),
	#[error(transparent)]
	GitRemote(#[from] gix::remote::init::Error),
	#[error(transparent)]
	GitConnect(#[from] gix::remote::connect::Error),
	#[error(transparent)]
	GitFetchPrepare(#[from] gix::remote::fetch::prepare::Error),
	#[error(transparent)]
	GitRemoteFetch(#[from] gix::remote::fetch::Error),
	#[error(transparent)]
	GitCloneFetch(#[from] gix::clone::fetch::Error),
	#[error(transparent)]
	GitFindObject(#[from] gix::object::find::existing::Error),
	#[error(transparent)]
	GitTraverse(#[from] gix::traverse::tree::breadthfirst::Error),
	#[error(transparent)]
	GitHead(#[from] gix::reference::head_id::Error),
	#[error(transparent)]
	GitCommit(#[from] gix::object::commit::Error),
	#[error(transparent)]
	GitRevparse(#[from] gix::revision::spec::parse::single::Error),
	#[error(transparent)]
	GitRefspec(#[from] gix::refspec::parse::Error),
	#[error(transparent)]
	GitPeel(#[from] gix::reference::peel::Error),
	#[error(transparent)]
	GitPeelToKind(#[from] gix::object::peel::to_kind::Error),
	#[error(transparent)]
	GitOpen(#[from] gix::open::Error),
	#[error("http error: {0}")]
	Http(#[from] reqwest::Error),
	#[error("zip error: {0}")]
	Zip(Box<zip::result::ZipError>),
	#[error(transparent)]
	Accessor(#[from] accessor::Error),
	#[error(
		"checksum mismatch for {dep}: expected {expected}, got {got}. If the lockfile was written by jsonnet-bundler, pass --compat to accept and rewrite its checksums"
	)]
	ChecksumMismatch {
		dep: Utf8PathBuf,
		expected: String,
		got: String,
	},
	#[error("unknown subdir: {0}")]
	SubdirNotFound(String),
	#[error("invalid path in tree: {0}")]
	InvalidPath(String),
}
pub(crate) type Result<T, E = Error> = result::Result<T, E>;
