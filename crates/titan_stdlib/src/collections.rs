//! Titan Stdlib — Collections.

pub type Vec<T> = std::vec::Vec<T>;
pub type HashMap<K,V> = std::collections::HashMap<K,V>;
pub type HashSet<T> = std::collections::HashSet<T>;

pub fn vec_new<T>() -> Vec<T> { Vec::new() }
pub fn vec_push<T>(v: &mut Vec<T>, el: T) { v.push(el); }
pub fn vec_pop<T>(v: &mut Vec<T>) -> Option<T> { v.pop() }
pub fn vec_len<T>(v: &Vec<T>) -> usize { v.len() }