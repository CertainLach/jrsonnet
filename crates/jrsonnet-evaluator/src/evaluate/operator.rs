use std::cmp::Ordering;

use jrsonnet_parser::{BinaryOpType, LocExpr, UnaryOpType};

use crate::{
	arr::ArrValue,
	bail,
	error::ErrorKind::*,
	evaluate,
	stdlib::std_format,
	typed::IntoUntyped,
	val::{equals, StrValue},
	Context, Result, Val,
};

pub fn evaluate_unary_op(op: UnaryOpType, b: &Val) -> Result<Val> {
	use UnaryOpType::*;
	use Val::*;
	Ok(match (op, b) {
		(Plus, Num(n)) => Val::Num(*n),
		(Minus, Num(n)) => Val::try_num(-n.get())?,
		(Not, Bool(v)) => Bool(!v),
		#[expect(
			clippy::cast_precision_loss,
			reason = "behavior is expected to be truncating"
		)]
		(BitNot, Num(n)) => Val::try_num(!(n.get() as i64) as f64)?,
		(op, o) => bail!(UnaryOperatorDoesNotOperateOnType(op, o.value_type())),
	})
}

pub fn evaluate_add_op(a: &Val, b: &Val) -> Result<Val> {
	use Val::*;
	Ok(match (a, b) {
		(Str(v1), Str(v2)) => Str(StrValue::concat(v1.clone(), v2.clone())),

		(Num(a), Str(b)) => Val::string(format!("{a}{b}")),
		(Str(a), Num(b)) => Val::string(format!("{a}{b}")),

		(Str(a), o) | (o, Str(a)) if a.is_empty() => Val::string(o.clone().to_string()?),
		(Str(a), o) => Val::string(format!("{a}{}", o.clone().to_string()?)),
		(o, Str(a)) => Val::string(format!("{}{a}", o.clone().to_string()?)),

		(Obj(v1), Obj(v2)) => Obj(v2.extend_from(v1.clone())),
		(Arr(a), Arr(b)) => Val::Arr(ArrValue::extended(a.clone(), b.clone())),

		(Num(v1), Num(v2)) => Val::try_num(v1.get() + v2.get())?,
		#[cfg(feature = "exp-bigint")]
		(BigInt(a), BigInt(b)) => BigInt(Box::new(&**a + &**b)),
		_ => bail!(BinaryOperatorDoesNotOperateOnValues(
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
			if b.get() == 0.0 {
				bail!(DivisionByZero)
			}
			Ok(Val::try_num(a.get() % b.get())?)
		}
		(Str(str), vals) => {
			String::into_untyped(std_format(&str.clone().into_flat(), vals.clone())?)
		}
		(a, b) => bail!(BinaryOperatorDoesNotOperateOnValues(
			BinaryOpType::Mod,
			a.value_type(),
			b.value_type()
		)),
	}
}

pub fn evaluate_binary_op_special(
	ctx: &Context,
	a: &LocExpr,
	op: BinaryOpType,
	b: &LocExpr,
) -> Result<Val> {
	use BinaryOpType::*;
	use Val::*;
	Ok(match (evaluate(ctx, a)?, op, b) {
		(Bool(true), Or, _o) => Val::Bool(true),
		(Bool(false), And, _o) => Val::Bool(false),
		#[cfg(feature = "exp-null-coaelse")]
		(Null, NullCoaelse, eb) => evaluate(ctx, eb)?,
		#[cfg(feature = "exp-null-coaelse")]
		(a, NullCoaelse, _o) => a,
		(a, op, eb) => evaluate_binary_op_normal(&a, op, &evaluate(ctx, eb)?)?,
	})
}

pub fn evaluate_compare_op(a: &Val, b: &Val, op: BinaryOpType) -> Result<Ordering> {
	use Val::*;
	Ok(match (a, b) {
		(Str(a), Str(b)) => a.cmp(b),
		(Num(a), Num(b)) => a.cmp(b),
		#[cfg(feature = "exp-bigint")]
		(BigInt(a), BigInt(b)) => a.cmp(b),
		(Arr(a), Arr(b)) => {
			let ai = a.iter();
			let bi = b.iter();

			for (a, b) in ai.zip(bi) {
				let ord = evaluate_compare_op(&a?, &b?, op)?;
				if !ord.is_eq() {
					return Ok(ord);
				}
			}
			a.len().cmp(&b.len())
		}
		(_, _) => bail!(BinaryOperatorDoesNotOperateOnValues(
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

		(a, Lt, b) => Bool(evaluate_compare_op(a, b, Lt)?.is_lt()),
		(a, Gt, b) => Bool(evaluate_compare_op(a, b, Gt)?.is_gt()),
		(a, Lte, b) => Bool(evaluate_compare_op(a, b, Lte)?.is_le()),
		(a, Gte, b) => Bool(evaluate_compare_op(a, b, Gte)?.is_ge()),

		(Str(a), In, Obj(obj)) => Bool(obj.has_field_ex(a.clone().into_flat(), true)),
		(a, Mod, b) => evaluate_mod_op(a, b)?,

		// Compat: This behavior is specific to jrsonnet, as it turns out, don't want to break compatibility
		#[expect(
			clippy::cast_sign_loss,
			reason = "multiply by negative doesn't make sense, but it wasn't erroring in the old versions"
		)]
		(Str(v1), Mul, Num(v2)) => Val::string(v1.to_string().repeat(v2.get() as usize)),

		// Bool X Bool
		(Bool(a), And, Bool(b)) => Bool(*a && *b),
		(Bool(a), Or, Bool(b)) => Bool(*a || *b),

		// Num X Num
		(Num(v1), Mul, Num(v2)) => Val::try_num(v1.get() * v2.get())?,
		(Num(v1), Div, Num(v2)) => {
			if v2.get() == 0.0 {
				bail!(DivisionByZero)
			}
			Val::try_num(v1.get() / v2.get())?
		}

		(Num(v1), Sub, Num(v2)) => Val::try_num(v1.get() - v2.get())?,

		(Num(v1), BitAnd, Num(v2)) => Val::try_num(v1.get_safe_int()? & v2.get_safe_int()?)?,
		(Num(v1), BitOr, Num(v2)) => Val::try_num(v1.get_safe_int()? | v2.get_safe_int()?)?,
		(Num(v1), BitXor, Num(v2)) => Val::try_num(v1.get_safe_int()? ^ v2.get_safe_int()?)?,
		#[expect(clippy::cast_sign_loss, reason = "rhs is checked to be non-negative")]
		(Num(lhs), Lhs, Num(rhs)) => {
			let lhs = lhs.get_safe_int()?;
			let mut rhs = rhs.get_safe_int()?;
			if rhs < 0 {
				bail!("shift by negative exponent")
			}
			rhs &= 63;
			Val::try_num(lhs.wrapping_shl(rhs as u32))?
		}
		#[expect(clippy::cast_sign_loss, reason = "rhs is checked to be non-negative")]
		(Num(lhs), Rhs, Num(rhs)) => {
			let lhs = lhs.get_safe_int()?;
			let mut rhs = rhs.get_safe_int()?;
			if rhs < 0 {
				bail!("shift by negative exponent")
			}
			rhs &= 63;
			Val::try_num(lhs.wrapping_shr(rhs as u32))?
		}

		// Bigint X Bigint
		#[cfg(feature = "exp-bigint")]
		(BigInt(a), Mul, BigInt(b)) => BigInt(Box::new(&**a * &**b)),
		#[cfg(feature = "exp-bigint")]
		(BigInt(a), Sub, BigInt(b)) => BigInt(Box::new(&**a - &**b)),

		_ => bail!(BinaryOperatorDoesNotOperateOnValues(
			op,
			a.value_type(),
			b.value_type(),
		)),
	})
}
