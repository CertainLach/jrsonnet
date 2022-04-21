use jrsonnet_parser::{BinaryOpType, LocExpr, UnaryOpType};

use crate::{
	builtin::std_format, error::Error::*, evaluate, throw, typed::Typed, val::equals, Context,
	Result, State, Val,
};

pub fn evaluate_unary_op(op: UnaryOpType, b: &Val) -> Result<Val> {
	use UnaryOpType::*;
	use Val::*;
	Ok(match (op, b) {
		(Not, Bool(v)) => Bool(!v),
		(Minus, Num(n)) => Num(-*n),
		(BitNot, Num(n)) => Num(!(*n as i32) as f64),
		(op, o) => throw!(UnaryOperatorDoesNotOperateOnType(op, o.value_type())),
	})
}

pub fn evaluate_add_op(s: State, a: &Val, b: &Val) -> Result<Val> {
	use Val::*;
	Ok(match (a, b) {
		(Str(v1), Str(v2)) => Str(((**v1).to_owned() + v2).into()),

		// Can't use generic json serialization way, because it depends on number to string concatenation (std.jsonnet:890)
		(Num(n), Str(o)) => Str(format!("{}{}", n, o).into()),
		(Str(o), Num(n)) => Str(format!("{}{}", o, n).into()),

		(Str(a), o) => Str(format!("{}{}", a, o.clone().to_string(s)?).into()),
		(o, Str(a)) => Str(format!("{}{}", o.clone().to_string(s)?, a).into()),

		(Obj(v1), Obj(v2)) => Obj(v2.extend_from(v1.clone())),
		(Arr(a), Arr(b)) => {
			let mut out = Vec::with_capacity(a.len() + b.len());
			out.extend(a.iter_lazy());
			out.extend(b.iter_lazy());
			Arr(out.into())
		}
		(Num(v1), Num(v2)) => Val::new_checked_num(v1 + v2)?,
		_ => throw!(BinaryOperatorDoesNotOperateOnValues(
			BinaryOpType::Add,
			a.value_type(),
			b.value_type(),
		)),
	})
}

pub fn evaluate_mod_op(s: State, a: &Val, b: &Val) -> Result<Val> {
	use Val::*;
	match (a, b) {
		(Num(a), Num(b)) => Ok(Num(a % b)),
		(Str(str), vals) => {
			String::into_untyped(std_format(s.clone(), str.clone(), vals.clone())?, s)
		}
		(a, b) => throw!(BinaryOperatorDoesNotOperateOnValues(
			BinaryOpType::Mod,
			a.value_type(),
			b.value_type()
		)),
	}
}

pub fn evaluate_binary_op_special(
	s: State,
	ctx: Context,
	a: &LocExpr,
	op: BinaryOpType,
	b: &LocExpr,
) -> Result<Val> {
	use BinaryOpType::*;
	use Val::*;
	Ok(match (evaluate(s.clone(), ctx.clone(), a)?, op, b) {
		(Bool(true), Or, _o) => Val::Bool(true),
		(Bool(false), And, _o) => Val::Bool(false),
		(a, op, eb) => evaluate_binary_op_normal(s.clone(), &a, op, &evaluate(s, ctx, eb)?)?,
	})
}

pub fn evaluate_binary_op_normal(s: State, a: &Val, op: BinaryOpType, b: &Val) -> Result<Val> {
	use BinaryOpType::*;
	use Val::*;
	Ok(match (a, op, b) {
		(a, Add, b) => evaluate_add_op(s, a, b)?,

		(a, Eq, b) => Bool(equals(s, a, b)?),
		(a, Neq, b) => Bool(!equals(s, a, b)?),

		(Str(a), In, Obj(obj)) => Bool(obj.has_field_ex(a.clone(), true)),
		(a, Mod, b) => evaluate_mod_op(s, a, b)?,

		(Str(v1), Mul, Num(v2)) => Str(v1.repeat(*v2 as usize).into()),

		// Bool X Bool
		(Bool(a), And, Bool(b)) => Bool(*a && *b),
		(Bool(a), Or, Bool(b)) => Bool(*a || *b),

		// Str X Str
		(Str(v1), Lt, Str(v2)) => Bool(v1 < v2),
		(Str(v1), Gt, Str(v2)) => Bool(v1 > v2),
		(Str(v1), Lte, Str(v2)) => Bool(v1 <= v2),
		(Str(v1), Gte, Str(v2)) => Bool(v1 >= v2),

		// Num X Num
		(Num(v1), Mul, Num(v2)) => Val::new_checked_num(v1 * v2)?,
		(Num(v1), Div, Num(v2)) => {
			if *v2 <= f64::EPSILON {
				throw!(DivisionByZero)
			}
			Val::new_checked_num(v1 / v2)?
		}

		(Num(v1), Sub, Num(v2)) => Val::new_checked_num(v1 - v2)?,

		(Num(v1), Lt, Num(v2)) => Bool(v1 < v2),
		(Num(v1), Gt, Num(v2)) => Bool(v1 > v2),
		(Num(v1), Lte, Num(v2)) => Bool(v1 <= v2),
		(Num(v1), Gte, Num(v2)) => Bool(v1 >= v2),

		(Num(v1), BitAnd, Num(v2)) => Num(((*v1 as i32) & (*v2 as i32)) as f64),
		(Num(v1), BitOr, Num(v2)) => Num(((*v1 as i32) | (*v2 as i32)) as f64),
		(Num(v1), BitXor, Num(v2)) => Num(((*v1 as i32) ^ (*v2 as i32)) as f64),
		(Num(v1), Lhs, Num(v2)) => {
			if *v2 < 0.0 {
				throw!(RuntimeError("shift by negative exponent".into()))
			}
			Num(((*v1 as i32) << (*v2 as i32)) as f64)
		}
		(Num(v1), Rhs, Num(v2)) => {
			if *v2 < 0.0 {
				throw!(RuntimeError("shift by negative exponent".into()))
			}
			Num(((*v1 as i32) >> (*v2 as i32)) as f64)
		}

		_ => throw!(BinaryOperatorDoesNotOperateOnValues(
			op,
			a.value_type(),
			b.value_type(),
		)),
	})
}
