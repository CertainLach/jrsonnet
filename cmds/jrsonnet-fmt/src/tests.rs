use dprint_core::formatting::{PrintItems, PrintOptions};
use indoc::indoc;

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
			indent_width: 2,
			max_width: 100,
			use_tabs: true,
			new_line_text: "\n",
		},
	)
}

#[test]
fn complex_comments_snapshot() {
	insta::assert_snapshot!(reformat(indoc!(
		"{
		  comments: {
			_: '',
			//     Plain comment
			a: '',

			#    Plain comment with empty line before
			b: '',
			/*Single-line multiline comment

			*/
			c: '',

			/**Single-line multiline doc comment

			*/
			c: '',

			/**Multiline doc
			Comment
			*/
			c: '',

			/*

	Multi-line

	comment
			*/
			d: '',

			e: '', // Inline comment

			k: '',

			// Text after everything
		  },
		  comments2: {
			k: '',
			// Text after everything, but no newline above
		  },
          spacing: {
            a: '',

            b: '',
          },
          noSpacing: {
            a: '',
            b: '',
          },
        }"
	)))
}
