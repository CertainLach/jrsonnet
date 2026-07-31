use std::{
	collections::BTreeSet,
	fs::{self, File},
	io::Read,
	path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use super::{Error, Result, VERSION_FILE};

/// Paths excluded from the hash of the directory they are nested in.
pub struct Exclusions(pub BTreeSet<PathBuf>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	/// Every file in the tree is hashed.
	Correct,
	/// Reproduces the checksums of `jsonnet-bundler`, which stops hashing the
	/// rest of the tree as soon as a file can't be read.
	JsonnetBundler,
}

/// Checksum of a vendored directory, compatible with `jsonnet-bundler`:
/// base64 of sha256 over the concatenated contents of every file, walked in
/// lexicographic order per directory.
///
/// `version` is the contents of the [`VERSION_FILE`] marker written by `jrb`;
/// the marker is not part of the hash, as `jsonnet-bundler` doesn't write it.
pub fn hash_dir(
	dir: &Path,
	exclude: &Exclusions,
	version: Option<&str>,
	mode: Mode,
) -> Result<String> {
	let mut hasher = Sha256::new();
	walk(dir, version, exclude, mode, &mut hasher)?;
	Ok(STANDARD.encode(hasher.finalize()))
}

/// Returns `false` if the walk was aborted the way `jsonnet-bundler` aborts it,
/// leaving the hash truncated.
fn walk(
	dir: &Path,
	version: Option<&str>,
	exclude: &Exclusions,
	mode: Mode,
	hasher: &mut Sha256,
) -> Result<bool> {
	let mut entries = fs::read_dir(dir)
		.and_then(Iterator::collect::<std::result::Result<Vec<_>, _>>)
		.map_err(|e| Error::Io(dir.to_owned(), e))?;
	entries.sort_by_key(fs::DirEntry::file_name);

	for entry in entries {
		let path = entry.path();
		if exclude.0.contains(&path) {
			continue;
		}
		let ty = entry.file_type().map_err(|e| Error::Io(path.clone(), e))?;
		if ty.is_dir() {
			if !walk(&path, None, exclude, mode, hasher)? {
				return Ok(false);
			}
			continue;
		}
		if entry.file_name() == VERSION_FILE && is_marker(&path, version) {
			continue;
		}
		// jsonnet-bundler follows symlinks, and stops hashing the whole tree if
		// the link doesn't resolve to a file.
		if ty.is_symlink() && !fs::metadata(&path).is_ok_and(|m| m.is_file()) {
			if mode == Mode::JsonnetBundler {
				warn!(
					"{}: symlink to a non-file, jsonnet-bundler truncates the checksum of this tree here",
					path.display()
				);
				return Ok(false);
			}
			debug!("{}: symlink to a non-file, not hashed", path.display());
			continue;
		}
		if let Err(e) = hash_file(&path, hasher) {
			if mode == Mode::Correct {
				return Err(Error::Io(path, e));
			}
			warn!(
				"{}: {e}, jsonnet-bundler stops hashing here",
				path.display()
			);
			return Ok(false);
		}
	}

	Ok(true)
}

/// Whether this [`VERSION_FILE`] was written by `jrb`, and not shipped by the
/// dependency itself.
fn is_marker(path: &Path, version: Option<&str>) -> bool {
	let Some(version) = version else {
		return false;
	};
	fs::read_to_string(path).is_ok_and(|v| v.trim() == version)
}

fn hash_file(path: &Path, hasher: &mut Sha256) -> std::io::Result<()> {
	let mut file = File::open(path)?;
	let mut buf = vec![0; 64 * 1024];
	loop {
		let read = file.read(&mut buf)?;
		if read == 0 {
			return Ok(());
		}
		hasher.update(&buf[..read]);
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use super::*;

	fn tempdir(name: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!("jrsonnet-pkg-hash-{name}"));
		let _ = fs::remove_dir_all(&dir);
		fs::create_dir_all(&dir).expect("create tempdir");
		dir
	}

	fn no_exclusions() -> Exclusions {
		Exclusions(BTreeSet::new())
	}

	#[test]
	fn matches_jsonnet_bundler() {
		let dir = tempdir("basic");
		fs::write(dir.join("b.libsonnet"), "b").expect("write");
		fs::write(dir.join("a.libsonnet"), "a").expect("write");
		fs::create_dir(dir.join("sub")).expect("mkdir");
		fs::write(dir.join("sub/c.libsonnet"), "c").expect("write");

		// sha256("abc"), as walked in lexicographic order
		assert_eq!(
			hash_dir(&dir, &no_exclusions(), None, Mode::Correct).expect("hash"),
			"ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
		);
	}

	#[test]
	fn version_marker_is_not_hashed() {
		let dir = tempdir("version");
		fs::write(dir.join("a.libsonnet"), "a").expect("write");
		fs::write(dir.join(VERSION_FILE), "deadbeef\n").expect("write");
		fs::create_dir(dir.join("sub")).expect("mkdir");
		fs::write(dir.join("sub").join(VERSION_FILE), "bc").expect("write");

		// sha256("abc"): the nested .version is a regular file for jsonnet-bundler
		assert_eq!(
			hash_dir(&dir, &no_exclusions(), Some("deadbeef"), Mode::Correct).expect("hash"),
			"ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
		);
	}

	#[test]
	fn shipped_version_file_is_hashed() {
		let dir = tempdir("version-shipped");
		fs::write(dir.join(VERSION_FILE), "a").expect("write");
		fs::write(dir.join("b.libsonnet"), "bc").expect("write");

		// sha256("abc")
		assert_eq!(
			hash_dir(&dir, &no_exclusions(), Some("deadbeef"), Mode::Correct).expect("hash"),
			"ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
		);
	}

	#[test]
	fn excluded_paths_are_skipped() {
		let dir = tempdir("exclude");
		fs::write(dir.join("a.libsonnet"), "abc").expect("write");
		fs::create_dir(dir.join("nested")).expect("mkdir");
		fs::write(dir.join("nested/d.libsonnet"), "d").expect("write");

		let exclude = Exclusions(std::iter::once(dir.join("nested")).collect());
		assert_eq!(
			hash_dir(&dir, &exclude, None, Mode::Correct).expect("hash"),
			"ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
		);
	}

	#[test]
	#[cfg(unix)]
	fn symlink_to_file_is_followed() {
		let dir = tempdir("symlink-file");
		fs::write(dir.join("a.libsonnet"), "abc").expect("write");
		std::os::unix::fs::symlink("a.libsonnet", dir.join("b.libsonnet")).expect("symlink");

		// sha256("abcabc")
		assert_eq!(
			hash_dir(&dir, &no_exclusions(), None, Mode::Correct).expect("hash"),
			"u7Wdo6+Tn3r182DyzrgKSW47rhzYfd5CbbCuQGd+HCw="
		);
	}

	#[test]
	#[cfg(unix)]
	fn symlink_to_dir_truncates_only_in_compat() {
		let dir = tempdir("symlink-dir");
		fs::write(dir.join("a.libsonnet"), "ab").expect("write");
		fs::create_dir(dir.join("sub")).expect("mkdir");
		fs::write(dir.join("sub/z.libsonnet"), "c").expect("write");
		std::os::unix::fs::symlink("sub", dir.join("b")).expect("symlink");

		// the symlink is skipped, sha256("abc")
		assert_eq!(
			hash_dir(&dir, &no_exclusions(), None, Mode::Correct).expect("hash"),
			"ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
		);
		// jsonnet-bundler stops at the symlink, never reaching sub/: sha256("ab")
		assert_eq!(
			hash_dir(&dir, &no_exclusions(), None, Mode::JsonnetBundler).expect("hash"),
			"+44g/C5MPySMYMOb1lLzwTRymLuXe4tNWQO4UFViBgM="
		);
	}
}
