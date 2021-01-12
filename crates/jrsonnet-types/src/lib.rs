use std::fmt::Display;

#[macro_export]
macro_rules! ty {
	((Array<number>)) => {{
		$crate::ComplexValType::ArrayRef(&$crate::ComplexValType::Simple($crate::ValType::Num))
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
		static CONTENTS: &'static [$crate::ComplexValType] = &[
			$(ty!($a)),+
		];
		$crate::ComplexValType::UnionRef(CONTENTS)
	}};
	(($($a:tt) &+)) => {{
		static CONTENTS: &'static [$crate::ComplexValType] = &[
			$(ty!($a)),+
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
			ComplexValType::Simple(ValType::Str),
			ComplexValType::Simple(ValType::Num)
		])
	);
	assert_eq!(
		format!("{}", ty!(((string & number) | (object & null)))),
		"string & number | object & null"
	);
	assert_eq!(format!("{}", ty!((string | array))), "string | array");
	assert_eq!(format!("{}", ty!(((string & number) | array))), "string & number | array");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
	Bool,
	Null,
	Str,
	Num,
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

#[derive(Debug, Clone, PartialEq)]
pub enum ComplexValType {
	Any,
	Char,
	Simple(ValType),
	BoundedNumber(Option<f64>, Option<f64>),
	ArrayRef(&'static ComplexValType),
	ObjectRef(&'static [(&'static str, ComplexValType)]),
	UnionRef(&'static [ComplexValType]),
	SumRef(&'static [ComplexValType]),
}
impl From<ValType> for ComplexValType {
	fn from(s: ValType) -> Self {
		Self::Simple(s)
	}
}

fn write_union(
	f: &mut std::fmt::Formatter<'_>,
	is_union: bool,
	union: &[ComplexValType],
) -> std::fmt::Result {
	for (i, v) in union.iter().enumerate() {
		let should_add_braces = match v {
			ComplexValType::UnionRef(_) if !is_union => true,
			_ => false,
		};
		if i != 0 {
			write!(f, " {} ", if is_union { '|' } else { '&' })?;
		}
		if should_add_braces {
			write!(f, "(")?;
		}
		write!(f, "{}", v)?;
		if should_add_braces {
			write!(f, ")")?;
		}
	}
	Ok(())
}

impl Display for ComplexValType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ComplexValType::Any => write!(f, "any")?,
			ComplexValType::Simple(s) => write!(f, "{}", s)?,
			ComplexValType::Char => write!(f, "char")?,
			ComplexValType::BoundedNumber(a, b) => write!(
				f,
				"BoundedNumber<{}, {}>",
				a.map(|e| e.to_string()).unwrap_or_else(|| "".into()),
				b.map(|e| e.to_string()).unwrap_or_else(|| "".into())
			)?,
			ComplexValType::ArrayRef(a) => {
				if **a == ComplexValType::Any {
					write!(f, "array")?
				} else {
					write!(f, "Array<{}>", a)?
				}
			}
			ComplexValType::ObjectRef(fields) => {
				write!(f, "{{")?;
				for (i, (k, v)) in fields.iter().enumerate() {
					if i != 0 {
						write!(f, ", ")?;
					}
					write!(f, "{}: {}", k, v)?;
				}
				write!(f, "}}")?;
			}
			ComplexValType::UnionRef(v) => write_union(f, true, v)?,
			ComplexValType::SumRef(v) => write_union(f, false, v)?,
		};
		Ok(())
	}
}
