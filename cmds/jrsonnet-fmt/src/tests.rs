use dprint_core::formatting::PrintOptions;
use indoc::indoc;

use crate::Printable;

fn reformat(input: &str) -> String {
	let (source, _) = jrsonnet_rowan_parser::parse(input);

	dprint_core::formatting::format(
		|| source.print(),
		PrintOptions {
			indent_width: 2,
			max_width: 100,
			use_tabs: false,
			new_line_text: "\n",
		},
	)
}

macro_rules! assert_formatted {
	($input:literal, $output:literal) => {
		let formatted = reformat(indoc!($input));
		let mut expected = indoc!($output).to_owned();
		expected.push('\n');
		if formatted != expected {
			panic!(
				"bad formatting, expected\n```\n{formatted}\n```\nto be equal to\n```\n{expected}\n```",
			)
		}
	};
}

#[test]
fn padding_stripped_for_multiline_comment() {
	assert_formatted!(
		"{
            /*
                Hello
                    World
            */
            _: null,
        }",
		"{
          /*
          Hello
              World
          */
          _: null,
        }"
	);
}

#[test]
fn last_comment_respects_spacing_with_inline_comment_above() {
	assert_formatted!(
		"{
			a: '', // Inline

			// Comment
        }",
		"{
		  a: '', // Inline

		  // Comment
		}"
	);
}

#[test]
fn complex_comments_snapshot() {
	insta::assert_display_snapshot!(reformat(indoc!(
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
