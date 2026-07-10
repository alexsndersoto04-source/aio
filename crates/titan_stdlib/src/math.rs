//! Titan Stdlib — Math.

pub const PI: f64 = std::f64::consts::PI;
pub const E: f64 = std::f64::consts::E;
pub fn sqrt(x: f64) -> f64 { x.sqrt() }
pub fn pow(b: f64, e: f64) -> f64 { b.powf(e) }
pub fn sin(x: f64) -> f64 { x.sin() }
pub fn cos(x: f64) -> f64 { x.cos() }
pub fn tan(x: f64) -> f64 { x.tan() }
pub fn ln(x: f64) -> f64 { x.ln() }
pub fn abs(x: f64) -> f64 { x.abs() }
pub fn round(x: f64) -> f64 { x.round() }
pub fn floor(x: f64) -> f64 { x.floor() }
pub fn ceil(x: f64) -> f64 { x.ceil() }
pub fn min<T: PartialOrd>(a: T, b: T) -> T { if a<b {a} else {b} }
pub fn max<T: PartialOrd>(a: T, b: T) -> T { if a>b {a} else {b} }
pub fn mean(values: &[f64]) -> f64 { if values.is_empty(){0.0} else {values.iter().sum::<f64>()/values.len() as f64} }