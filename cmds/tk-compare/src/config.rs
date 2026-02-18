use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
	#[serde(default)]
	pub working_dir: Option<String>,
	#[serde(default)]
	pub tests: Vec<TestDefinition>,
}

#[derive(Debug, Deserialize)]
pub struct TestDefinition {
	pub fixtures_dir: String,
	pub args: Vec<String>,
	#[serde(default)]
	pub compare: Vec<String>,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub working_dir: Option<String>,
	#[serde(default = "default_workspace")]
	pub workspace: bool,
	#[serde(default)]
	pub fixture_name_prefix: Option<String>,
	#[serde(default)]
	pub fixture_cluster_dir: Option<String>,
	#[serde(default)]
	pub expect_error: bool,
	#[serde(default)]
	pub rtk_config: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FixtureConfig {
	#[serde(default)]
	extra_args: Vec<String>,
	#[serde(default)]
	args: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct FixtureConfigLayer {
	#[serde(default)]
	extra_args: Option<Vec<String>>,
	#[serde(default)]
	args: Option<BTreeMap<String, Vec<String>>>,
}

impl FixtureConfig {
	fn merge_layer(&mut self, layer: FixtureConfigLayer) {
		if let Some(extra_args) = layer.extra_args {
			self.extra_args.extend(extra_args);
		}
		if let Some(args) = layer.args {
			for (command, new_args) in args {
				self.args.entry(command).or_default().extend(new_args);
			}
		}
	}

	fn args_for_command(&self, command: Option<&str>) -> Vec<String> {
		let mut args = self.extra_args.clone();
		if let Some(command) = command {
			if let Some(command_args) = self.args.get(command) {
				args.extend(command_args.clone());
			}
		}
		args
	}
}

fn default_workspace() -> bool {
	true
}

#[derive(Debug, Clone, Deserialize)]
pub struct Command {
	pub args: Vec<String>,
	pub compare: Vec<String>,
	#[serde(default)]
	pub name: Option<String>,
	#[serde(default)]
	pub expect_error: bool,
	#[serde(default)]
	pub rtk_config: Option<String>,
	#[serde(default)]
	pub cluster_dir: Option<String>,
}

pub struct ResolvedCommand {
	pub command: Command,
	pub working_dir: Option<String>,
	pub workspace: bool,
	pub basename: String,
	pub testcase: String,
	pub test_name: String,
}

impl Config {
	pub fn from_file(path: &str) -> Result<Self> {
		let contents = std::fs::read_to_string(path)
			.with_context(|| format!("Failed to read config file: {}", path))?;
		let mut config: Config = toml::from_str(&contents)
			.with_context(|| format!("Failed to parse config file: {}", path))?;

		if let Some(ref wd) = config.working_dir {
			config.working_dir = Some(expand_env_vars(wd));
		}

		for test in &mut config.tests {
			test.fixtures_dir = expand_env_vars(&test.fixtures_dir);
			if let Some(ref wd) = test.working_dir {
				test.working_dir = Some(expand_env_vars(wd));
			}
			if let Some(ref cluster_dir) = test.fixture_cluster_dir {
				test.fixture_cluster_dir = Some(expand_env_vars(cluster_dir));
			}
			test.args = test.args.iter().map(|arg| expand_env_vars(arg)).collect();
			test.compare = test
				.compare
				.iter()
				.map(|arg| expand_env_vars(arg))
				.collect();
		}

		Ok(config)
	}

	pub fn all_commands(&self) -> Result<Vec<ResolvedCommand>> {
		let nested = self
			.tests
			.iter()
			.map(|test| self.resolve_test(test))
			.collect::<Result<Vec<_>>>()?;
		Ok(nested.into_iter().flatten().collect())
	}

	fn resolve_test(&self, test: &TestDefinition) -> Result<Vec<ResolvedCommand>> {
		if test.args.is_empty() {
			bail!("Each [[tests]] entry must define a non-empty `args` array");
		}

		let configured_working_dir = test
			.working_dir
			.clone()
			.or_else(|| self.working_dir.clone());
		let scan_working_dir = configured_working_dir
			.as_deref()
			.filter(|wd| !has_template_tokens(wd))
			.map(std::string::ToString::to_string)
			.or_else(|| self.working_dir.clone());
		let fixtures_path = resolve_scan_path(&test.fixtures_dir, scan_working_dir.as_deref());

		let mut entries: Vec<_> = std::fs::read_dir(&fixtures_path)
			.with_context(|| format!("Failed to read fixtures_dir: {}", fixtures_path.display()))?
			.filter_map(|entry| entry.ok())
			.filter(|entry| entry.path().is_dir())
			.collect();
		entries.sort_by_key(|entry| entry.path());

		let base_name = test.name.clone().unwrap_or_else(|| "fixtures".to_string());
		let fixture_name_prefix = test
			.fixture_name_prefix
			.clone()
			.unwrap_or_else(|| "case".to_string());

		let resolved: Vec<_> = entries
			.into_iter()
			.map(|entry| {
				let fixture_path = entry.path();
				let basename = entry.file_name().to_string_lossy().to_string();
				let testcase = path_for_command_arg(&fixture_path, scan_working_dir.as_deref());
				let resolved_working_dir = configured_working_dir
					.as_ref()
					.map(|wd| render_tokens(wd, &testcase, &basename));

				let mut args: Vec<String> = test
					.args
					.iter()
					.map(|arg| render_tokens(arg, &testcase, &basename))
					.collect();
				let fixture_args = load_fixture_args(&fixtures_path, &fixture_path)?
					.args_for_command(args.first().map(std::string::String::as_str));
				args.extend(
					fixture_args
						.into_iter()
						.map(|arg| render_tokens(&arg, &testcase, &basename)),
				);

				let cluster_dir = test.fixture_cluster_dir.as_ref().and_then(|dir_name| {
					let cluster_path = if dir_name == "." {
						fixture_path.clone()
					} else {
						fixture_path.join(dir_name)
					};
					cluster_path
						.is_dir()
						.then(|| path_for_command_arg(&cluster_path, scan_working_dir.as_deref()))
				});

				let command = Command {
					args,
					compare: compare_argv(&test.compare),
					name: Some(format!("{}: {}", fixture_name_prefix, basename)),
					expect_error: test.expect_error,
					rtk_config: test.rtk_config.clone(),
					cluster_dir,
				};

				Ok(ResolvedCommand {
					command,
					working_dir: resolved_working_dir,
					workspace: test.workspace,
					basename,
					testcase,
					test_name: base_name.clone(),
				})
			})
			.collect::<Result<Vec<_>>>()?;

		if resolved.is_empty() {
			bail!(
				"No fixture cases found in {} (expected child directories)",
				fixtures_path.display()
			);
		}

		Ok(resolved)
	}
}

fn load_fixture_args(
	suite_path: &std::path::Path,
	fixture_path: &std::path::Path,
) -> Result<FixtureConfig> {
	let mut config = FixtureConfig::default();
	let mut config_dirs = vec![suite_path];
	if fixture_path != suite_path {
		config_dirs.push(fixture_path);
	}

	for config_dir in config_dirs {
		let config_path = config_dir.join("tk-compare.toml");
		if !config_path.exists() {
			continue;
		}

		let contents = std::fs::read_to_string(&config_path)
			.with_context(|| format!("Failed to read fixture config: {}", config_path.display()))?;
		let layer: FixtureConfigLayer = toml::from_str(&contents).with_context(|| {
			format!("Failed to parse fixture config: {}", config_path.display())
		})?;
		config.merge_layer(layer);
	}

	Ok(config)
}

pub fn load_fixture_command_args(
	suite_path: &std::path::Path,
	fixture_path: &std::path::Path,
	command: &str,
	testcase: &str,
	basename: &str,
) -> Result<Vec<String>> {
	let args = load_fixture_args(suite_path, fixture_path).map(|cfg| {
		cfg.args_for_command(Some(command))
			.into_iter()
			.map(|arg| render_tokens(&arg, testcase, basename))
			.collect::<Vec<_>>()
	})?;
	Ok(args)
}

fn resolve_scan_path(path: &str, working_dir: Option<&str>) -> std::path::PathBuf {
	let path = std::path::PathBuf::from(path);
	if path.is_absolute() {
		return path;
	}
	if let Some(wd) = working_dir {
		return std::path::Path::new(wd).join(path);
	}
	path
}

fn path_for_command_arg(path: &std::path::Path, working_dir: Option<&str>) -> String {
	if let Some(wd) = working_dir {
		let wd_path = std::path::Path::new(wd);
		if let Ok(relative) = path.strip_prefix(wd_path) {
			return relative.to_string_lossy().to_string();
		}
	}
	path.to_string_lossy().to_string()
}

fn has_template_tokens(s: &str) -> bool {
	s.contains("{{testcase}}") || s.contains("{{basename}}")
}

fn render_tokens(value: &str, testcase: &str, basename: &str) -> String {
	value
		.replace("{{testcase}}", testcase)
		.replace("{{basename}}", basename)
}

fn compare_argv(configured: &[String]) -> Vec<String> {
	if !configured.is_empty() {
		return configured.to_vec();
	}
	vec![
		"{{tk-compare}}".to_string(),
		"compare".to_string(),
		"auto".to_string(),
	]
}

fn expand_env_vars(s: &str) -> String {
	let mut result = s.to_string();

	while let Some(start) = result.find("${") {
		if let Some(end) = result[start..].find('}') {
			let var_name = &result[start + 2..start + end];
			let value = std::env::var(var_name).unwrap_or_default();
			result.replace_range(start..start + end + 1, &value);
		} else {
			break;
		}
	}

	let chars = result.chars().collect::<Vec<_>>();
	let mut i = 0;
	let mut new_result = String::new();

	while i < chars.len() {
		if chars[i] == '$'
			&& i + 1 < chars.len()
			&& (chars[i + 1].is_alphabetic() || chars[i + 1] == '_')
		{
			let var_start = i + 1;
			let mut var_end = var_start;
			while var_end < chars.len()
				&& (chars[var_end].is_alphanumeric() || chars[var_end] == '_')
			{
				var_end += 1;
			}
			let var_name: String = chars[var_start..var_end].iter().collect();
			let value = std::env::var(&var_name).unwrap_or_default();
			new_result.push_str(&value);
			i = var_end;
		} else {
			new_result.push(chars[i]);
			i += 1;
		}
	}

	new_result
}

pub const RTK_CONFIG_FILENAME: &str = ".rtk-config.yaml";

impl Command {
	pub fn as_string(&self) -> String {
		self.args.join(" ")
	}

	pub fn display_name(&self) -> String {
		self.name.clone().unwrap_or_else(|| self.as_string())
	}

	/// Supports: {{destination}}, {{basename}}, {{testcase}}, {{tempdir}}, {{working_dir}}
	pub fn args_for_exec(
		&self,
		destination: &str,
		basename: &str,
		testcase: &str,
		tempdir: &str,
		working_dir: Option<&str>,
	) -> Vec<String> {
		self.args
			.iter()
			.map(|arg| {
				let replaced = arg
					.replace("{{destination}}", destination)
					.replace("{{basename}}", basename)
					.replace("{{testcase}}", testcase)
					.replace("{{tempdir}}", tempdir);
				if let Some(wd) = working_dir {
					replaced.replace("{{working_dir}}", wd)
				} else {
					replaced
				}
			})
			.collect()
	}

	/// Supports: {{tk-compare}}, {{basename}}, {{testcase}}, {{tempdir}}
	pub fn compare_argv(
		&self,
		tk_compare: &str,
		basename: &str,
		testcase: &str,
		tempdir: &str,
	) -> Vec<String> {
		self.compare
			.iter()
			.map(|arg| {
				arg.replace("{{tk-compare}}", tk_compare)
					.replace("{{basename}}", basename)
					.replace("{{testcase}}", testcase)
					.replace("{{tempdir}}", tempdir)
			})
			.collect()
	}

	pub fn write_rtk_config(&self, working_dir: Option<&str>) -> Option<std::path::PathBuf> {
		let config_content = self.rtk_config.as_ref()?;
		let dir = working_dir?;
		let config_path = std::path::Path::new(dir).join(RTK_CONFIG_FILENAME);
		if std::fs::write(&config_path, config_content).is_ok() {
			Some(config_path)
		} else {
			eprintln!(
				"Warning: Failed to write rtk config to {}",
				config_path.display()
			);
			None
		}
	}

	pub fn cleanup_rtk_config(working_dir: Option<&str>) {
		if let Some(dir) = working_dir {
			let config_path = std::path::Path::new(dir).join(RTK_CONFIG_FILENAME);
			if config_path.exists() {
				let _ = std::fs::remove_file(&config_path);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use indoc::indoc;

	use super::*;

	#[test]
	fn test_args_template_substitution() {
		let cmd = Command {
			args: vec![
				"diff".to_string(),
				"{{testcase}}/environment".to_string(),
				"{{destination}}".to_string(),
			],
			compare: vec![],
			name: None,
			expect_error: false,
			rtk_config: None,
			cluster_dir: None,
		};

		let args = cmd.args_for_exec(
			"/tmp/t/outputs/rtk/case-a",
			"case-a",
			"fixtures/case-a",
			"/tmp/t",
			None,
		);
		assert_eq!(
			args,
			vec![
				"diff",
				"fixtures/case-a/environment",
				"/tmp/t/outputs/rtk/case-a"
			]
		);
	}

	#[test]
	fn test_compare_template_substitution() {
		let cmd = Command {
			args: vec![],
			compare: vec![
				"{{tk-compare}}".to_string(),
				"compare".to_string(),
				"json".to_string(),
				"{{basename}}".to_string(),
			],
			name: None,
			expect_error: false,
			rtk_config: None,
			cluster_dir: None,
		};

		let compare = cmd.compare_argv("/bin/tk-compare", "case-a", "fixtures/case-a", "/tmp/t");
		assert_eq!(
			compare,
			vec!["/bin/tk-compare", "compare", "json", "case-a"]
		);
	}

	#[test]
	fn test_fixture_expansion_uses_testcase_directory() {
		let temp_dir = std::env::temp_dir().join(format!(
			"tk_compare_fixtures_template_test_{}",
			std::process::id()
		));
		let fixture_dir = temp_dir.join("fixtures").join("a");
		let _ = std::fs::remove_dir_all(&temp_dir);
		std::fs::create_dir_all(&fixture_dir).unwrap();

		let config: Config = toml::from_str(
			&indoc! {r#"
				working_dir = "{working_dir}"

				[[tests]]
				fixtures_dir = "fixtures"
				args = ["diff", "{{testcase}}/environment"]
				compare = ["{{tk-compare}}", "compare", "unified-diff"]
			"#}
			.replace("{working_dir}", &temp_dir.to_string_lossy()),
		)
		.unwrap();

		let commands = config.all_commands().unwrap();
		assert_eq!(
			commands
				.iter()
				.map(|c| {
					(
						c.test_name.clone(),
						c.basename.clone(),
						c.testcase.clone(),
						c.command.args.clone(),
						c.command.compare.clone(),
						c.command.expect_error,
						c.command.cluster_dir.clone(),
					)
				})
				.collect::<Vec<_>>(),
			vec![(
				"fixtures".to_string(),
				"a".to_string(),
				"fixtures/a".to_string(),
				vec!["diff".to_string(), "fixtures/a/environment".to_string()],
				vec![
					"{{tk-compare}}".to_string(),
					"compare".to_string(),
					"unified-diff".to_string()
				],
				false,
				None
			)]
		);

		let _ = std::fs::remove_dir_all(&temp_dir);
	}

	#[test]
	fn test_fixture_extra_args_are_appended() {
		let temp_dir = std::env::temp_dir().join(format!(
			"tk_compare_fixture_extra_args_test_{}",
			std::process::id()
		));
		let fixture_dir = temp_dir.join("fixtures").join("a");
		let _ = std::fs::remove_dir_all(&temp_dir);
		std::fs::create_dir_all(&fixture_dir).unwrap();
		std::fs::write(
			fixture_dir.join("tk-compare.toml"),
			"extra_args = [\"--with-prune\", \"--name={{basename}}\"]\n",
		)
		.unwrap();

		let config: Config = toml::from_str(
			&indoc! {r#"
				working_dir = "{working_dir}"

				[[tests]]
				fixtures_dir = "fixtures"
				args = ["diff", "{{testcase}}/environment"]
			"#}
			.replace("{working_dir}", &temp_dir.to_string_lossy()),
		)
		.unwrap();

		let commands = config.all_commands().unwrap();
		assert_eq!(
			commands
				.iter()
				.map(|c| {
					(
						c.test_name.clone(),
						c.basename.clone(),
						c.testcase.clone(),
						c.command.args.clone(),
						c.command.compare.clone(),
						c.command.expect_error,
						c.command.cluster_dir.clone(),
					)
				})
				.collect::<Vec<_>>(),
			vec![(
				"fixtures".to_string(),
				"a".to_string(),
				"fixtures/a".to_string(),
				vec![
					"diff".to_string(),
					"fixtures/a/environment".to_string(),
					"--with-prune".to_string(),
					"--name=a".to_string(),
				],
				vec![
					"{{tk-compare}}".to_string(),
					"compare".to_string(),
					"auto".to_string()
				],
				false,
				None
			)]
		);

		let _ = std::fs::remove_dir_all(&temp_dir);
	}

	#[test]
	fn test_fixture_extra_args_can_be_loaded_from_suite_dir() {
		let temp_dir = std::env::temp_dir().join(format!(
			"tk_compare_fixture_suite_args_test_{}",
			std::process::id()
		));
		let suite_dir = temp_dir.join("fixtures");
		let fixture_dir = suite_dir.join("a");
		let _ = std::fs::remove_dir_all(&temp_dir);
		std::fs::create_dir_all(&fixture_dir).unwrap();
		std::fs::write(
			suite_dir.join("tk-compare.toml"),
			indoc! {r#"
				[args]
				diff = ["--suite-flag", "--name={{basename}}"]
			"#},
		)
		.unwrap();

		let config: Config = toml::from_str(
			&indoc! {r#"
				working_dir = "{working_dir}"

				[[tests]]
				fixtures_dir = "fixtures"
				args = ["diff", "{{testcase}}/environment"]
			"#}
			.replace("{working_dir}", &temp_dir.to_string_lossy()),
		)
		.unwrap();

		let commands = config.all_commands().unwrap();
		assert_eq!(
			commands
				.iter()
				.map(|c| c.command.args.clone())
				.collect::<Vec<_>>(),
			vec![vec![
				"diff".to_string(),
				"fixtures/a/environment".to_string(),
				"--suite-flag".to_string(),
				"--name=a".to_string(),
			]]
		);

		let _ = std::fs::remove_dir_all(&temp_dir);
	}

	#[test]
	fn test_fixture_extra_args_extend_suite_config() {
		let temp_dir = std::env::temp_dir().join(format!(
			"tk_compare_fixture_override_args_test_{}",
			std::process::id()
		));
		let suite_dir = temp_dir.join("fixtures");
		let fixture_dir = suite_dir.join("a");
		let _ = std::fs::remove_dir_all(&temp_dir);
		std::fs::create_dir_all(&fixture_dir).unwrap();
		std::fs::write(
			suite_dir.join("tk-compare.toml"),
			indoc! {r#"
				extra_args = ["--suite-extra"]
				[args]
				export = ["--extension", "golden", "--parallel", "1"]
			"#},
		)
		.unwrap();
		std::fs::write(
			fixture_dir.join("tk-compare.toml"),
			indoc! {r#"
				extra_args = ["--fixture-extra", "--name={{basename}}"]
				[args]
				export = ["--extension", "yaml"]
			"#},
		)
		.unwrap();

		let config: Config = toml::from_str(
			&indoc! {r#"
				working_dir = "{working_dir}"

				[[tests]]
				fixtures_dir = "fixtures"
				args = ["export", "{{testcase}}/environment"]
			"#}
			.replace("{working_dir}", &temp_dir.to_string_lossy()),
		)
		.unwrap();

		let commands = config.all_commands().unwrap();
		assert_eq!(
			commands
				.iter()
				.map(|c| c.command.args.clone())
				.collect::<Vec<_>>(),
			vec![vec![
				"export".to_string(),
				"fixtures/a/environment".to_string(),
				"--suite-extra".to_string(),
				"--fixture-extra".to_string(),
				"--name=a".to_string(),
				"--extension".to_string(),
				"golden".to_string(),
				"--parallel".to_string(),
				"1".to_string(),
				"--extension".to_string(),
				"yaml".to_string(),
			]]
		);

		let _ = std::fs::remove_dir_all(&temp_dir);
	}
}
