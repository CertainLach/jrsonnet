use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
	pub tk_exec_1: String,
	pub tk_exec_2: String,
	#[serde(default)]
	pub working_dir: Option<String>,
	pub commands: Vec<Command>,
}

#[derive(Debug, Deserialize)]
pub struct Command {
	pub args: Vec<String>,
	#[serde(default)]
	pub result_dir: Option<String>,
	#[serde(default = "default_runs")]
	pub runs: usize,
}

fn default_runs() -> usize {
	1
}

impl Config {
	pub fn from_file(path: &str) -> Result<Self> {
		let contents = std::fs::read_to_string(path)
			.with_context(|| format!("Failed to read config file: {}", path))?;
		let mut config: Config = toml::from_str(&contents)
			.with_context(|| format!("Failed to parse config file: {}", path))?;

		// Expand environment variables in string fields
		config.tk_exec_1 = expand_env_vars(&config.tk_exec_1);
		config.tk_exec_2 = expand_env_vars(&config.tk_exec_2);
		if let Some(ref wd) = config.working_dir {
			config.working_dir = Some(expand_env_vars(wd));
		}

		Ok(config)
	}
}

/// Expand environment variables in a string
/// Supports ${VAR} and $VAR syntax
fn expand_env_vars(s: &str) -> String {
	let mut result = s.to_string();

	// Handle ${VAR} syntax
	while let Some(start) = result.find("${") {
		if let Some(end) = result[start..].find('}') {
			let var_name = &result[start + 2..start + end];
			let value = std::env::var(var_name).unwrap_or_default();
			result.replace_range(start..start + end + 1, &value);
		} else {
			break;
		}
	}

	// Handle $VAR syntax (word boundary terminated)
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

impl Command {
	pub fn as_string(&self) -> String {
		self.args.join(" ")
	}
}
