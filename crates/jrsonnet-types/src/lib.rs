use std::fmt::Display;

#[macro_export]
macro_rules! ty {
	([$inner:tt]) => {{
		static VAL: &'static ComplexValType = &ty!($inner);
		match VAL {
			ComplexValType::Any => ComplexValType::Simple(ValType::Arr),
			_ => ComplexValType::ArrayRef(&VAL),
		}
	}};
	(bool) => {
		ComplexValType::Simple(ValType::Bool)
	};
	(null) => {
		ComplexValType::Simple(ValType::Null)
	};
	(str) => {
		ComplexValType::Simple(ValType::Str)
	};
	(char) => {
		ComplexValType::Char
	};
	(num) => {
		ComplexValType::Simple(ValType::Num)
	};
	(number(($min:expr)..($max:expr))) => {{
		ComplexValType::BoundedNumber($min, $max)
	}};
	(obj) => {
		ComplexValType::Simple(ValType::Obj)
	};
	(any) => {
		ComplexValType::Any
	};
	(fn.any) => {
		ComplexValType::Simple(ValType::Func)
	};
	(($($a:tt) |+)) => {{
		static CONTENTS: &'static [ComplexValType] = &[
			$(ty!($a)),+
		];
		ComplexValType::UnionRef(CONTENTS)
	}};
	(($($a:tt) &+)) => {{
		static CONTENTS: &'static [ComplexValType] = &[
			$(ty!($a)),+
		];
		ComplexValType::SumRef(CONTENTS)
	}};
}

#[test]
fn test() {
	assert_eq!(
		ty!([num]),
		ComplexValType::ArrayRef(&ComplexValType::Simple(ValType::Num))
	);
	assert_eq!(ty!([any]), ComplexValType::Simple(ValType::Arr));
	assert_eq!(ty!(any), ComplexValType::Any);
	assert_eq!(
		ty!((str | num)),
		ComplexValType::UnionRef(&[
			ComplexValType::Simple(ValType::Str),
			ComplexValType::Simple(ValType::Num)
		])
	);
	assert_eq!(
		format!("{}", ty!(((str & num) | (obj & null)))),
		"((str & num) | (obj & null))"
	);
	assert_eq!(format!("{}", ty!((str | [any]))), "(str | [any])");
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
			Bool => "bool",
			Null => "null",
			Str => "str",
			Num => "num",
			Arr => "[any]",
			Obj => "obj",
			Func => "fn.any",
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

impl ComplexValType {
	fn needs_brackets(&self) -> bool {
		matches!(self, ComplexValType::UnionRef(_) | ComplexValType::SumRef(_))
	}
}

fn write_union(
	f: &mut std::fmt::Formatter<'_>,
	ch: char,
	union: &[ComplexValType],
) -> std::fmt::Result {
	write!(f, "(")?;
	for (i, v) in union.iter().enumerate() {
		if i != 0 {
			write!(f, " {} ", ch)?;
		}
		write!(f, "{}", v)?;
	}
	write!(f, ")")?;
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
				"number({}..{})",
				a.map(|e| e.to_string()).unwrap_or_else(|| "".into()),
				b.map(|e| e.to_string()).unwrap_or_else(|| "".into())
			)?,
			ComplexValType::ArrayRef(a) => {
				if a.needs_brackets() {
					write!(f, "(")?;
				}
				write!(f, "{}", a)?;
				if a.needs_brackets() {
					write!(f, ")")?;
				}
				write!(f, "[]")?;
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
			ComplexValType::UnionRef(v) => write_union(f, '|', v)?,
			ComplexValType::SumRef(v) => write_union(f, '&', v)?,
		};
		Ok(())
	}
}
