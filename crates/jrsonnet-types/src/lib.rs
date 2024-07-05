#![allow(clippy::redundant_closure_call)]

use std::fmt::Display;

use jrsonnet_gcmodule::Trace;

#[macro_export]
macro_rules! ty {
	((Array<number>)) => {{
		$crate::ComplexValType::ArrayRef(&$crate::ComplexValType::Simple($crate::ValType::Num))
	}};
	((Array<ubyte>)) => {{
		$crate::ComplexValType::ArrayRef(&$crate::ComplexValType::BoundedNumber(Some(0.0), Some(255.0)))
	}};
	(array) => {
		$crate::ComplexValType::Simple($crate::ValType::Arr)
	};
	(boolean) => {
		$crate::ComplexValType::Simple($crate::ValType::Bool)
	};
	(null) => {
		$crate::ComplexValType::Simple($crate::ValType::Null)
	};
	(string) => {
		$crate::ComplexValType::Simple($crate::ValType::Str)
	};
	(char) => {
		$crate::ComplexValType::Char
	};
	(number) => {
		$crate::ComplexValType::Simple($crate::ValType::Num)
	};
	(BoundedNumber<($min:expr), ($max:expr)>) => {{
		$crate::ComplexValType::BoundedNumber($min, $max)
	}};
	(object) => {
		$crate::ComplexValType::Simple($crate::ValType::Obj)
	};
	(any) => {
		$crate::ComplexValType::Any
	};
	(function) => {
		$crate::ComplexValType::Simple($crate::ValType::Func)
	};
	(($($a:tt) |+)) => {{
		static CONTENTS: &'static [&'static $crate::ComplexValType] = &[
			$(&ty!($a)),+
		];
		$crate::ComplexValType::UnionRef(CONTENTS)
	}};
	(($($a:tt) &+)) => {{
		static CONTENTS: &'static [&'static $crate::ComplexValType] = &[
			$(&ty!($a)),+
		];
		$crate::ComplexValType::SumRef(CONTENTS)
	}};
}

#[test]
fn test() {
	assert_eq!(
		ty!((Array<number>)),
		ComplexValType::ArrayRef(&ComplexValType::Simple(ValType::Num))
	);
	assert_eq!(ty!(array), ComplexValType::Simple(ValType::Arr));
	assert_eq!(ty!(any), ComplexValType::Any);
	assert_eq!(
		ty!((string | number)),
		ComplexValType::UnionRef(&[
			&ComplexValType::Simple(ValType::Str),
			&ComplexValType::Simple(ValType::Num)
		])
	);
	assert_eq!(
		format!("{}", ty!(((string & number) | (object & null)))),
		"string & number | object & null"
	);
	assert_eq!(format!("{}", ty!((string | array))), "string | array");
	assert_eq!(
		format!("{}", ty!(((string & number) | array))),
		"string & number | array"
	);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Trace)]
pub enum ValType {
	Bool,
	Null,
	Str,
	Num,
	#[cfg(feature = "exp-bigint")]
	BigInt,
	Arr,
	Obj,
	Func,
}

impl ValType {
	pub const fn name(&self) -> &'static str {
		use ValType::*;
		match self {
			Bool => "boolean",
			Null => "null",
			Str => "string",
			Num => "number",
			#[cfg(feature = "exp-bigint")]
			BigInt => "bigint",
			Arr => "array",
			Obj => "object",
			Func => "function",
		}
	}
}

impl Display for ValType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.name())
	}
}

#[derive(Debug, Clone, PartialEq, Trace)]
#[trace(skip)]
pub enum ComplexValType {
	Any,
	Char,
	Simple(ValType),
	BoundedNumber(Option<f64>, Option<f64>),
	Array(Box<ComplexValType>),
	ArrayRef(&'static ComplexValType),
	ObjectRef(&'static [(&'static str, &'static ComplexValType)]),
	AttrsOf(&'static ComplexValType),
	Union(Vec<ComplexValType>),
	UnionRef(&'static [&'static ComplexValType]),
	Sum(Vec<ComplexValType>),
	SumRef(&'static [&'static ComplexValType]),
	Lazy(&'static ComplexValType),
}

impl From<ValType> for ComplexValType {
	fn from(s: ValType) -> Self {
		Self::Simple(s)
	}
}

fn write_union<'i>(
	f: &mut std::fmt::Formatter<'_>,
	is_union: bool,
	union: impl Iterator<Item = &'i ComplexValType>,
) -> std::fmt::Result {
	for (i, v) in union.enumerate() {
		let should_add_braces =
			matches!(v, ComplexValType::UnionRef(_) | ComplexValType::Union(_) if !is_union);
		if i != 0 {
			write!(f, " {} ", if is_union { '|' } else { '&' })?;
		}
		if should_add_braces {
			write!(f, "(")?;
		}
		write!(f, "{v}")?;
		if should_add_braces {
			write!(f, ")")?;
		}
	}
	Ok(())
}

fn print_array(a: &ComplexValType, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
	if *a == ComplexValType::Any {
		write!(f, "array")?;
	} else {
		write!(f, "Array<{a}>")?;
	}
	Ok(())
}

impl Display for ComplexValType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Any => write!(f, "any")?,
			Self::Simple(s) => write!(f, "{s}")?,
			Self::Char => write!(f, "char")?,
			Self::BoundedNumber(a, b) => write!(
				f,
				"BoundedNumber<{}, {}>",
				a.map(|e| e.to_string())
					.unwrap_or_else(|| "open".to_owned()),
				b.map(|e| e.to_string())
					.unwrap_or_else(|| "open".to_owned())
			)?,
			Self::ArrayRef(a) => print_array(a, f)?,
			Self::Array(a) => print_array(a, f)?,
			Self::ObjectRef(fields) => {
				write!(f, "{{")?;
				for (i, (k, v)) in fields.iter().enumerate() {
					if i != 0 {
						write!(f, ", ")?;
					}
					write!(f, "{k}: {v}")?;
				}
				write!(f, "}}")?;
			}
			Self::AttrsOf(a) => {
				if matches!(a, Self::Any) {
					write!(f, "object")?;
				} else {
					write!(f, "AttrsOf<{a}>")?;
				}
			}
			Self::Union(v) => write_union(f, true, v.iter())?,
			Self::UnionRef(v) => write_union(f, true, v.iter().copied())?,
			Self::Sum(v) => write_union(f, false, v.iter())?,
			Self::SumRef(v) => write_union(f, false, v.iter().copied())?,
			Self::Lazy(lazy) => write!(f, "Lazy<{lazy}>")?,
		};
		Ok(())
	}
}

#[cfg(test)]
pub mod tests {
	use super::parser;

	#[test]
	fn precedence() {
		assert_eq!(
			parser::ty("(any & any) | (any | any) & any")
				.unwrap()
				.to_string(),
			"any & any | (any | any) & any"
		);
	}

	#[test]
	fn array() {
		assert_eq!(parser::ty("Array<any>").unwrap().to_string(), "array");
		assert_eq!(
			parser::ty("Array<number>").unwrap().to_string(),
			"Array<number>"
		);
	}
	#[test]
	fn bounded_number() {
		assert_eq!(
			parser::ty("BoundedNumber<1, 2>").unwrap().to_string(),
			"BoundedNumber<1, 2>"
		);
	}
}
