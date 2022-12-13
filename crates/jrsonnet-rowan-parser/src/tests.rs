#![cfg(test)]

use miette::{
	Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, ThemeCharacters, ThemeStyles,
};
use thiserror::Error;

use crate::{parse, AstNode};

#[derive(Debug, Error)]
#[error("syntax error")]
struct MyDiagnostic {
	code: String,
	spans: Vec<LabeledSpan>,
}
impl Diagnostic for MyDiagnostic {
	fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
		None
	}

	fn severity(&self) -> Option<miette::Severity> {
		None
	}

	fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
		None
	}

	fn url<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
		None
	}

	fn source_code(&self) -> Option<&dyn miette::SourceCode> {
		Some(&self.code)
	}

	fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
		Some(Box::new(self.spans.clone().into_iter()))
	}

	fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
		None
	}
}

fn process(text: &str) -> String {
	use std::fmt::Write;
	let mut out = String::new();
	let (node, errors) = parse(text);
	write!(out, "{:#?}", node.syntax()).unwrap();
	if !errors.is_empty() && !text.is_empty() {
		writeln!(out, "===").unwrap();
		for err in &errors {
			writeln!(out, "{:?}", err).unwrap();
		}
		let mut code = text.to_string();

		// Prettier errors at EOF position
		if code.ends_with('\n') {
			code.truncate(code.len() - 1);
			code += " ";
		}
		code += " ";

		let diag = MyDiagnostic {
			code,
			spans: errors.into_iter().map(|e| e.into()).collect(),
		};

		let handler = GraphicalReportHandler::new_themed(GraphicalTheme {
			characters: ThemeCharacters::ascii(),
			styles: ThemeStyles::none(),
		});

		write!(out, "===").unwrap();
		handler
			.render_report(&mut out, &diag)
			.expect("fmt error?..");
	}
	out.split('\n')
		.map(|s| s.trim_end().to_string())
		.collect::<Vec<String>>()
		.join("\n")
		.trim_end()
		.to_string()
}
macro_rules! mk_test {
		($($name:ident => $test:expr)+) => {$(
			#[test]
			fn $name() {
				let src = indoc::indoc!($test);
				let result = process(&src);
				insta::assert_snapshot!(stringify!($name), result, src);

			}
		)+};
	}
mk_test!(
	empty => r#" "#
	function => r#"
		function(a, b = 1) a + b
	"#
	function_error_no_value => r#"
		function(a, b = ) a + b
	"#
	function_error_rparen => r#"
		function(a, b
	"#
	function_error_body => r#"
		function(a, b)
	"#
	local_novalue => r#"
		local a =
	"#
	local_no_value_recovery => r#"
		local a =
		local b = 3;
		1
	"#

	array_comp => r#"
		[a for a in [1, 2, 3]]
	"#
	array_comp_incompatible_with_multiple_elems => r#"
		[a for a in [1, 2, 3], b]
	"#

	no_rhs => r#"
		a +
	"#
	no_lhs => r#"
		+ 2
	"#
	no_operator => "
		2 2
	"

	named_before_positional => "
		a(1, 2, b=4, 3, 5, k = 12, 6)
	"

	wrong_field_end => "
		{
			a: 1;
			b: 2;
		}
	"


	plain_call => "
		std.substr(a, 0, std.length(b)) == b
	"

	destruct => "
		local [a, b, c] = arr;
		local [a, ...] = arr_rest;
		local [..., a] = rest_arr;
		local [...] = rest_in_arr;
		local [a, ...n] = arr_rest_n;
		local [...n, a] = rest_arr_n;
		local [...n] = rest_in_arr_n;

		local {a, b, c} = obj;
		local {a, b, c, ...} = obj_rest;
		local {a, b, c, ...n} = obj_rest_n;

		null
	"

	str_block_missing_indent => "
		|||
	"
	str_block_missing_termination => "
		|||
			hello
	"
	str_block_missing_newline => "
		|||hello
	"
	str_block_missing_indent_text => "
		|||
		hello
	"

	unexpected_destruct => "
		local * = 1;
		a
	"
);

#[test]
fn stdlib() {
	let src = include_str!("../../jrsonnet-stdlib/src/std.jsonnet");
	let result = process(src);
	insta::assert_snapshot!("stdlib", result, src);
}
