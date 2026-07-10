//! Titan Stdlib — Testing.

pub fn assert_eq<T: std::fmt::Debug + PartialEq>(left: T, right: T, msg: &str) {
    if left != right { panic!("assert failed: {} (left={:?} right={:?})", msg, left, right); }
}
pub fn assert(cond: bool, msg: &str) { if !cond { panic!("assert failed: {}", msg); } }
pub fn assert_ok<T, E: std::fmt::Debug>(r: Result<T,E>, msg: &str) -> T {
    match r { Ok(v) => v, Err(_) => panic!("expected Ok: {}", msg) }
}