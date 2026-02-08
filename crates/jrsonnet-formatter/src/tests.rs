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
fn complex_comments() {
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
	)));
}

#[test]
fn args() {
	insta::assert_snapshot!(reformat(indoc!(
		"
			{
				short: aaa(1,2,3,4,5),
				long: bbb(123123123123123123123,12312312321123123123,123123123123312123123,123123123123123123312,123123123123312321123),
				short_in_long: bbb(aaa(1,2,3,4,5), 123123123123123123123,12312312321123123123,123123123123312123123,123123123123123123312,123123123123312321123),
				long_in_short: aaa(1,2,3,4,5,bbb(123123123123123123123,12312312321123123123,123123123123312123123,123123123123123123312,123123123123312321123)),
			}
		"
	)));
}

#[test]
fn asserts() {
	insta::assert_snapshot!(reformat(indoc!(
		"
			{
				assert 1 > 0 : 'one should be greater than zero',
				assert true,
				value: 42,
			}
		"
	)));
}
