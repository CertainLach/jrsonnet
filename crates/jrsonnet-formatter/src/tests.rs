#![cfg(test)]

use std::fs;

use dprint_core::formatting::{PrintItems, PrintOptions};
use indoc::indoc;
use insta::{assert_snapshot, glob};

use crate::Printable;

fn reformat(input: &str) -> String {
	let (source, _) = jrsonnet_rowan_parser::parse(input);

	dprint_core::formatting::format(
		|| {
			let mut out = PrintItems::new();
			source.print(&mut out);
			out
		},
		PrintOptions {
			indent_width: 3,
			max_width: 100,
			use_tabs: false,
			new_line_text: "\n",
		},
	)
}

#[test]
fn snapshots() {
	glob!("tests/*.jsonnet", |path| {
		let input = fs::read_to_string(path).expect("read test file");
		assert_snapshot!(reformat(&input));
	});
}
