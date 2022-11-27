use jrsonnet_evaluator::{error::Result, function::builtin, typed::PositiveF64};

#[builtin]
pub fn builtin_abs(n: f64) -> Result<f64> {
	Ok(n.abs())
}

#[builtin]
pub fn builtin_sign(n: f64) -> Result<f64> {
	Ok(if n == 0. { 0. } else { n.signum() })
}

#[builtin]
pub fn builtin_max(a: f64, b: f64) -> Result<f64> {
	Ok(a.max(b))
}

#[builtin]
pub fn builtin_min(a: f64, b: f64) -> Result<f64> {
	Ok(a.min(b))
}

#[builtin]
pub fn builtin_modulo(a: f64, b: f64) -> Result<f64> {
	Ok(a % b)
}

#[builtin]
pub fn builtin_floor(x: f64) -> Result<f64> {
	Ok(x.floor())
}

#[builtin]
pub fn builtin_ceil(x: f64) -> Result<f64> {
	Ok(x.ceil())
}

#[builtin]
pub fn builtin_log(n: f64) -> Result<f64> {
	Ok(n.ln())
}

#[builtin]
pub fn builtin_pow(x: f64, n: f64) -> Result<f64> {
	Ok(x.powf(n))
}

#[builtin]
pub fn builtin_sqrt(x: PositiveF64) -> Result<f64> {
	Ok(x.0.sqrt())
}

#[builtin]
pub fn builtin_sin(x: f64) -> Result<f64> {
	Ok(x.sin())
}

#[builtin]
pub fn builtin_cos(x: f64) -> Result<f64> {
	Ok(x.cos())
}

#[builtin]
pub fn builtin_tan(x: f64) -> Result<f64> {
	Ok(x.tan())
}

#[builtin]
pub fn builtin_asin(x: f64) -> Result<f64> {
	Ok(x.asin())
}

#[builtin]
pub fn builtin_acos(x: f64) -> Result<f64> {
	Ok(x.acos())
}

#[builtin]
pub fn builtin_atan(x: f64) -> Result<f64> {
	Ok(x.atan())
}

#[builtin]
pub fn builtin_exp(x: f64) -> Result<f64> {
	Ok(x.exp())
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
pub fn builtin_mantissa(x: f64) -> Result<f64> {
	Ok(frexp(x).0)
}

#[builtin]
pub fn builtin_exponent(x: f64) -> Result<i16> {
	Ok(frexp(x).1)
}
