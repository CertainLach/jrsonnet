use std::cmp::Ordering;

use jrsonnet_parser::{BinaryOpType, LocExpr, UnaryOpType};

use crate::{
	error::Error::*, evaluate, stdlib::std_format, throw, typed::Typed, val::equals, Context,
	Result, Val,
};

pub fn evaluate_unary_op(op: UnaryOpType, b: &Val) -> Result<Val> {
	use UnaryOpType::*;
	use Val::*;
	Ok(match (op, b) {
		(Not, Bool(v)) => Bool(!v),
		(Minus, Num(n)) => Num(-*n),
		(BitNot, Num(n)) => Num(f64::from(!(*n as i32))),
		(op, o) => throw!(UnaryOperatorDoesNotOperateOnType(op, o.value_type())),
	})
}

pub fn evaluate_add_op(a: &Val, b: &Val) -> Result<Val> {
	use Val::*;
	Ok(match (a, b) {
		(Str(a), Str(b)) if a.is_empty() => Val::Str(b.clone()),
		(Str(a), Str(b)) if b.is_empty() => Val::Str(a.clone()),
		(Str(v1), Str(v2)) => Str(((**v1).to_owned() + v2).into()),

		// Can't use generic json serialization way, because it depends on number to string concatenation (std.jsonnet:890)
		(Num(a), Str(b)) => Str(format!("{a}{b}").into()),
		(Str(a), Num(b)) => Str(format!("{a}{b}").into()),

		(Str(a), o) | (o, Str(a)) if a.is_empty() => Val::Str(o.clone().to_string()?),
		(Str(a), o) => Str(format!("{a}{}", o.clone().to_string()?).into()),
		(o, Str(a)) => Str(format!("{}{a}", o.clone().to_string()?).into()),

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

pub fn evaluate_mod_op(a: &Val, b: &Val) -> Result<Val> {
	use Val::*;
	match (a, b) {
		(Num(a), Num(b)) => {
			if *b == 0.0 {
				throw!(DivisionByZero)
			}
			Ok(Num(a % b))
		}
		(Str(str), vals) => String::into_untyped(std_format(str.clone(), vals.clone())?),
		(a, b) => throw!(BinaryOperatorDoesNotOperateOnValues(
			BinaryOpType::Mod,
			a.value_type(),
			b.value_type()
		)),
	}
}

pub fn evaluate_binary_op_special(
	ctx: Context,
	a: &LocExpr,
	op: BinaryOpType,
	b: &LocExpr,
) -> Result<Val> {
	use BinaryOpType::*;
	use Val::*;
	Ok(match (evaluate(ctx.clone(), a)?, op, b) {
		(Bool(true), Or, _o) => Val::Bool(true),
		(Bool(false), And, _o) => Val::Bool(false),
		(a, op, eb) => evaluate_binary_op_normal(&a, op, &evaluate(ctx, eb)?)?,
	})
}

pub fn evaluate_compare_op(a: &Val, op: BinaryOpType, b: &Val) -> Result<Ordering> {
	use Val::*;
	Ok(match (a, b) {
		(Str(a), Str(b)) => a.cmp(b),
		(Num(a), Num(b)) => a.partial_cmp(b).expect("jsonnet numbers are non NaN"),
		(Arr(a), Arr(b)) => {
			let ai = a.iter();
			let bi = b.iter();

			for (a, b) in ai.zip(bi) {
				let ord = evaluate_compare_op(&a?, op, &b?)?;
				if !ord.is_eq() {
					return Ok(ord);
				}
			}

			a.len().cmp(&b.len())
		}
		(_, _) => throw!(BinaryOperatorDoesNotOperateOnValues(
			op,
			a.value_type(),
			b.value_type()
		)),
	})
}

pub fn evaluate_binary_op_normal(a: &Val, op: BinaryOpType, b: &Val) -> Result<Val> {
	use BinaryOpType::*;
	use Val::*;
	Ok(match (a, op, b) {
		(a, Add, b) => evaluate_add_op(a, b)?,

		(a, Eq, b) => Bool(equals(a, b)?),
		(a, Neq, b) => Bool(!equals(a, b)?),

		(a, Lt, b) => Bool(evaluate_compare_op(a, Lt, b)?.is_lt()),
		(a, Gt, b) => Bool(evaluate_compare_op(a, Gt, b)?.is_gt()),
		(a, Lte, b) => Bool(evaluate_compare_op(a, Lte, b)?.is_le()),
		(a, Gte, b) => Bool(evaluate_compare_op(a, Gte, b)?.is_ge()),

		(Str(a), In, Obj(obj)) => Bool(obj.has_field_ex(a.clone(), true)),
		(a, Mod, b) => evaluate_mod_op(a, b)?,

		(Str(v1), Mul, Num(v2)) => Str(v1.repeat(*v2 as usize).into()),

		// Bool X Bool
		(Bool(a), And, Bool(b)) => Bool(*a && *b),
		(Bool(a), Or, Bool(b)) => Bool(*a || *b),

		// Num X Num
		(Num(v1), Mul, Num(v2)) => Val::new_checked_num(v1 * v2)?,
		(Num(v1), Div, Num(v2)) => {
			if *v2 == 0.0 {
				throw!(DivisionByZero)
			}
			Val::new_checked_num(v1 / v2)?
		}

		(Num(v1), Sub, Num(v2)) => Val::new_checked_num(v1 - v2)?,

		(Num(v1), BitAnd, Num(v2)) => Num(f64::from((*v1 as i32) & (*v2 as i32))),
		(Num(v1), BitOr, Num(v2)) => Num(f64::from((*v1 as i32) | (*v2 as i32))),
		(Num(v1), BitXor, Num(v2)) => Num(f64::from((*v1 as i32) ^ (*v2 as i32))),
		(Num(v1), Lhs, Num(v2)) => {
			if *v2 < 0.0 {
				throw!("shift by negative exponent")
			}
			Num(f64::from((*v1 as i32) << (*v2 as i32)))
		}
		(Num(v1), Rhs, Num(v2)) => {
			if *v2 < 0.0 {
				throw!("shift by negative exponent")
			}
			Num(f64::from((*v1 as i32) >> (*v2 as i32)))
		}

		_ => throw!(BinaryOperatorDoesNotOperateOnValues(
			op,
			a.value_type(),
			b.value_type(),
		)),
	})
}
