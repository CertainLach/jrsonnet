use std::collections::BTreeSet;
use std::process::exit;

use clap::Parser;
use jrsonnet_evaluator::{FileImportResolver, ImportResolver};
use jrsonnet_ir::{visit::Visitor, IStr, Source, SourcePath};
use jrsonnet_ir_parser::ParserSettings;

use jrsonnet_cli::MiscOpts;

#[derive(Parser)]
struct Opts {
	/// Path to the file to start dependency search from
	input: String,
	#[clap(flatten)]
	misc: MiscOpts,
}

struct FoundImports(Vec<(IStr, bool)>);
impl Visitor for FoundImports {
	fn visit_import(&mut self, expression: bool, value: IStr) {
		self.0.push((value, expression));
	}
}

fn collect_deps(
	resolver: &FileImportResolver,
	source: &SourcePath,
	deps: &mut BTreeSet<String>,
) -> Result<(), String> {
	let contents = resolver
		.load_file_contents(source)
		.map_err(|e| format!("{e}"))?;
	let code = std::str::from_utf8(&contents).map_err(|e| format!("{source}: {e}"))?;
	let code: IStr = code.into();
	let parsed = jrsonnet_ir_parser::parse(
		&code,
		&ParserSettings {
			source: Source::new(source.clone(), code.clone()),
		},
	)
	.map_err(|e| format!("{source}: {e}"))?;

	let mut imports = FoundImports(vec![]);
	imports.visit_expr(&parsed);

	for (path, expression) in imports.0 {
		let resolved = resolver
			.resolve_from(source, &&*path)
			.map_err(|e| format!("{e}"))?;
		let path_str = format!("{resolved}");
		if deps.insert(path_str) && expression {
			collect_deps(resolver, &resolved, deps)?;
		}
	}

	Ok(())
}

fn main() {
	let opts = Opts::parse();
	let resolver = opts.misc.import_resolver();

	let source = resolver
		.resolve_from_default(&opts.input.as_str())
		.unwrap_or_else(|e| {
			eprintln!("{e}");
			exit(1);
		});

	let mut deps = BTreeSet::new();
	if let Err(e) = collect_deps(&resolver, &source, &mut deps) {
		eprintln!("{e}");
		exit(1);
	}

	for dep in &deps {
		println!("{dep}");
	}
}
