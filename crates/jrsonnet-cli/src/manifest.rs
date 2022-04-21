use std::{path::PathBuf, str::FromStr};

use clap::Parser;
use jrsonnet_evaluator::{error::Result, ManifestFormat, State};

use crate::ConfigureState;

pub enum ManifestFormatName {
	/// Expect string as output, and write them directly
	String,
	Json,
	Yaml,
}

impl FromStr for ManifestFormatName {
	type Err = &'static str;
	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		Ok(match s {
			"string" => ManifestFormatName::String,
			"json" => ManifestFormatName::Json,
			"yaml" => ManifestFormatName::Yaml,
			_ => return Err("no such format"),
		})
	}
}

#[derive(Parser)]
#[clap(next_help_heading = "MANIFESTIFICATION OUTPUT")]
pub struct ManifestOpts {
	/// Output format, wraps resulting value to corresponding std.manifest call.
	/// If set to `string` then plain string value is expected to be returned,
	/// otherwise output will be serialized to the specified format.
	#[clap(long, short = 'f', default_value = "json", possible_values = &["string", "json", "yaml"])]
	format: ManifestFormatName,
	/// Expect plain string as output.
	/// Shortcut for `--format=string` thus this option is mutually exclusive with `format` option.
	#[clap(long, short = 'S')]
	string: bool,
	/// Write output as YAML stream, can be used with --format json/yaml
	#[clap(long, short = 'y')]
	yaml_stream: bool,
	/// Number of spaces to pad output manifest with.
	/// `0` for hard tabs, `-1` for single line output [default: 3 for json, 2 for yaml]
	#[clap(long)]
	line_padding: Option<usize>,
	/// Preserve order in object manifestification
	#[cfg(feature = "exp-preserve-order")]
	#[clap(long)]
	exp_preserve_order: bool,
}
impl ConfigureState for ManifestOpts {
	fn configure(&self, s: &State) -> Result<()> {
		if self.string {
			s.set_manifest_format(ManifestFormat::String);
		} else {
			#[cfg(feature = "exp-preserve-order")]
			let preserve_order = self.exp_preserve_order;
			match self.format {
				ManifestFormatName::String => s.set_manifest_format(ManifestFormat::String),
				ManifestFormatName::Json => s.set_manifest_format(ManifestFormat::Json {
					padding: self.line_padding.unwrap_or(3),
					#[cfg(feature = "exp-preserve-order")]
					preserve_order,
				}),
				ManifestFormatName::Yaml => s.set_manifest_format(ManifestFormat::Yaml {
					padding: self.line_padding.unwrap_or(2),
					#[cfg(feature = "exp-preserve-order")]
					preserve_order,
				}),
			}
		}
		if self.yaml_stream {
			s.set_manifest_format(ManifestFormat::YamlStream(Box::new(s.manifest_format())))
		}
		Ok(())
	}
}

#[derive(Parser)]
pub struct OutputOpts {
	/// Write to the output file rather than stdout
	#[clap(long, short = 'o')]
	pub output_file: Option<PathBuf>,
	/// Automatically creates all parent directories for files
	#[clap(long, short = 'c')]
	pub create_output_dirs: bool,
	/// Write multiple files to the directory, list files on stdout
	#[clap(long, short = 'm')]
	pub multi: Option<PathBuf>,
}
