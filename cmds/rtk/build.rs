use std::process::Command;

fn main() {
	// Re-run if git HEAD changes
	println!("cargo:rerun-if-changed=.git/HEAD");
	println!("cargo:rerun-if-changed=.git/refs/");

	let version = get_version();
	println!("cargo:rustc-env=RTK_VERSION={}", version);
}

fn get_version() -> String {
	let cargo_version = env!("CARGO_PKG_VERSION");

	// If Cargo.toml version was set by release workflow (not placeholder), use it
	if cargo_version != "0.1.0" {
		return cargo_version.to_string();
	}

	// For local/dev builds, try git tag first
	if let Some(tag) = get_git_tag() {
		// Strip 'v' prefix if present (v0.0.16 -> 0.0.16)
		return tag.strip_prefix('v').unwrap_or(&tag).to_string();
	}

	// Fall back to commit hash for dev builds
	if let Some(commit) = get_git_commit() {
		return commit;
	}

	// Ultimate fallback
	cargo_version.to_string()
}

fn get_git_tag() -> Option<String> {
	// Check if HEAD points exactly to a tag
	let output = Command::new("git")
		.args(["describe", "--tags", "--exact-match", "HEAD"])
		.output()
		.ok()?;

	if output.status.success() {
		let tag = String::from_utf8(output.stdout).ok()?;
		Some(tag.trim().to_string())
	} else {
		None
	}
}

fn get_git_commit() -> Option<String> {
	let output = Command::new("git")
		.args(["rev-parse", "--short", "HEAD"])
		.output()
		.ok()?;

	if output.status.success() {
		let commit = String::from_utf8(output.stdout).ok()?;
		Some(commit.trim().to_string())
	} else {
		None
	}
}
