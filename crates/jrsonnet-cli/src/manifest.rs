use crate::ConfigureState;
use clap::Clap;
use jrsonnet_evaluator::{error::Result, EvaluationState, ManifestFormat};
use std::str::FromStr;

pub enum ManifestFormatName {
	/// Expect string as output, and write them directly
	None,
	Json,
	Yaml,
}

impl FromStr for ManifestFormatName {
	type Err = &'static str;
	fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
		Ok(match s {
			"none" => ManifestFormatName::None,
			"json" => ManifestFormatName::Json,
			"yaml" => ManifestFormatName::Yaml,
			_ => return Err("no such format"),
		})
	}
}

#[derive(Clap)]
// TODO: Restore arg group after issue fixed in clap
// #[clap(group = clap::ArgGroup::new("output_format"), help_heading = "MANIFESTIFICATION OUTPUT")]
pub struct ManifestOpts {
	/// Output format, wraps resulting value to corresponding std.manifest call.
	/// If none - then jsonnet file is expected to return plain string value, otherwise
	/// output will be serialized to specified format
	#[clap(long, short = 'f', default_value = "json", possible_values = &["none", "json", "yaml"]/*, group = "output_format"*/)]
	format: ManifestFormatName,
	/// Expect string as output, and write them directly.
	/// Shortcut for --format=none, and can't be set with format together
	#[clap(long, short = 'S'/*, group = "output_format"*/)]
	string: bool,
	/// Numbed of spaces to pad output manifest with.
	/// 0 for hard tabs, -1 for single line output
	#[clap(long, default_value = "3")]
	line_padding: usize,
}
impl ConfigureState for ManifestOpts {
	fn configure(&self, state: &EvaluationState) -> Result<()> {
		if self.string {
			state.set_manifest_format(ManifestFormat::None);
		} else {
			match self.format {
				ManifestFormatName::None => state.set_manifest_format(ManifestFormat::None),
				ManifestFormatName::Json => {
					state.set_manifest_format(ManifestFormat::Json(self.line_padding))
				}
				ManifestFormatName::Yaml => {
					state.set_manifest_format(ManifestFormat::Yaml(self.line_padding))
				}
			}
		}
		Ok(())
	}
}
