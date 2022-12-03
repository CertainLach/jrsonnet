use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use jrsonnet_evaluator::{
	error::Result,
	manifest::{JsonFormat, ManifestFormat, StringFormat, ToStringFormat, YamlStreamFormat},
	State,
};
use jrsonnet_stdlib::{TomlFormat, YamlFormat};

use crate::ConfigureState;

#[derive(Clone, ValueEnum)]
pub enum ManifestFormatName {
	/// Expect string as output, and write them directly
	String,
	Json,
	Yaml,
	Toml,
}

#[derive(Parser)]
#[clap(next_help_heading = "MANIFESTIFICATION OUTPUT")]
pub struct ManifestOpts {
	/// Output format, wraps resulting value to corresponding std.manifest call.
	#[clap(long, short = 'f', default_value = "json")]
	format: ManifestFormatName,
	/// Expect plain string as output.
	/// Mutually exclusive with `--format`
	#[clap(long, short = 'S', conflicts_with = "format")]
	string: bool,
	/// Write output as YAML stream, can be used with --format json/yaml
	#[clap(long, short = 'y', conflicts_with = "string")]
	yaml_stream: bool,
	/// Number of spaces to pad output manifest with.
	/// `0` for hard tabs, `-1` for single line output [default: 3 for json, 2 for yaml/toml]
	#[clap(long)]
	line_padding: Option<usize>,
	/// Preserve order in object manifestification
	#[cfg(feature = "exp-preserve-order")]
	#[clap(long)]
	pub preserve_order: bool,
}
impl ConfigureState for ManifestOpts {
	type Guards = Box<dyn ManifestFormat>;
	fn configure(&self, _s: &State) -> Result<Self::Guards> {
		let format: Box<dyn ManifestFormat> = if self.string {
			Box::new(StringFormat)
		} else {
			#[cfg(feature = "exp-preserve-order")]
			let preserve_order = self.preserve_order;
			match self.format {
				ManifestFormatName::String => Box::new(ToStringFormat),
				ManifestFormatName::Json => Box::new(JsonFormat::cli(
					self.line_padding.unwrap_or(3),
					#[cfg(feature = "exp-preserve-order")]
					preserve_order,
				)),
				ManifestFormatName::Yaml => Box::new(YamlFormat::cli(
					self.line_padding.unwrap_or(2),
					#[cfg(feature = "exp-preserve-order")]
					preserve_order,
				)),
				ManifestFormatName::Toml => Box::new(TomlFormat::cli(
					self.line_padding.unwrap_or(2),
					#[cfg(feature = "exp-preserve-order")]
					preserve_order,
				)),
			}
		};
		Ok(if self.yaml_stream {
			Box::new(YamlStreamFormat(format))
		} else {
			format
		})
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
