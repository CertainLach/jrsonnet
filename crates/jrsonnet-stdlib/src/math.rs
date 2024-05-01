use jrsonnet_evaluator::{function::builtin, typed::PositiveF64};

#[builtin]
pub fn builtin_abs(n: f64) -> f64 {
	n.abs()
}

#[builtin]
pub fn builtin_sign(n: f64) -> f64 {
	if n == 0. {
		0.
	} else {
		n.signum()
	}
}

#[builtin]
pub fn builtin_max(a: f64, b: f64) -> f64 {
	a.max(b)
}

#[builtin]
pub fn builtin_min(a: f64, b: f64) -> f64 {
	a.min(b)
}

#[allow(non_snake_case)]
#[builtin]
pub fn builtin_clamp(x: f64, minVal: f64, maxVal: f64) -> f64 {
	x.clamp(minVal, maxVal)
}

#[builtin]
pub fn builtin_sum(arr: Vec<f64>) -> f64 {
	arr.iter().sum()
}

#[builtin]
pub fn builtin_modulo(x: f64, y: f64) -> f64 {
	x % y
}

#[builtin]
pub fn builtin_floor(x: f64) -> f64 {
	x.floor()
}

#[builtin]
pub fn builtin_ceil(x: f64) -> f64 {
	x.ceil()
}

#[builtin]
pub fn builtin_log(x: f64) -> f64 {
	x.ln()
}

#[builtin]
pub fn builtin_pow(x: f64, n: f64) -> f64 {
	x.powf(n)
}

#[builtin]
pub fn builtin_sqrt(x: PositiveF64) -> f64 {
	x.0.sqrt()
}

#[builtin]
pub fn builtin_sin(x: f64) -> f64 {
	x.sin()
}

#[builtin]
pub fn builtin_cos(x: f64) -> f64 {
	x.cos()
}

#[builtin]
pub fn builtin_tan(x: f64) -> f64 {
	x.tan()
}

#[builtin]
pub fn builtin_asin(x: f64) -> f64 {
	x.asin()
}

#[builtin]
pub fn builtin_acos(x: f64) -> f64 {
	x.acos()
}

#[builtin]
pub fn builtin_atan(x: f64) -> f64 {
	x.atan()
}

#[builtin]
pub fn builtin_atan2(y: f64, x: f64) -> f64 {
	y.atan2(x)
}

#[builtin]
pub fn builtin_exp(x: f64) -> f64 {
	x.exp()
}

fn frexp(s: f64) -> (f64, i16) {
	if s == 0.0 {
		(s, 0)
	} else {
		let lg = s.abs().log2();
		let x = (lg - lg.floor() - 1.0).exp2();
		let exp = lg.floor() + 1.0;
		(s.signum() * x, exp as i16)
	}
}

#[builtin]
pub fn builtin_mantissa(x: f64) -> f64 {
	frexp(x).0
}

#[builtin]
pub fn builtin_exponent(x: f64) -> i16 {
	frexp(x).1
}

#[builtin]
pub fn builtin_round(x: f64) -> f64 {
	x.round()
}

#[builtin]
pub fn builtin_is_even(x: f64) -> bool {
	builtin_round(x) % 2.0 == 0.0
}

#[builtin]
#[allow(clippy::float_cmp)]
pub fn builtin_is_odd(x: f64) -> bool {
	builtin_round(x) % 2.0 == 1.0
}

#[builtin]
#[allow(clippy::float_cmp)]
pub fn builtin_is_integer(x: f64) -> bool {
	builtin_round(x) == x
}

#[builtin]
#[allow(clippy::float_cmp)]
pub fn builtin_is_decimal(x: f64) -> bool {
	builtin_round(x) != x
}
