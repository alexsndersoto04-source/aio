//! Phase 41: real C-ABI foreign function interface for `extern "C" fn ...;`.
//!
//! No crates, no wrappers: symbols are resolved at runtime with
//! `dlopen`/`dlsym` (Linux/Android/macOS) or `LoadLibraryA`/
//! `GetProcAddress` (Windows) and called through a fixed-arity prototype.
//!
//! ## What is real
//!
//! - The shared library is really loaded and the symbol really looked up.
//!   A missing library or symbol is a typed `VmError::Native`, never a
//!   silent no-op.
//! - Arguments are really marshalled into the C ABI: `int`/`bool`/`char`
//!   travel as machine-word integer-class registers (8 bytes on 64-bit
//!   targets, 4 bytes on 32-bit ARM — the fixed-arity prototypes below are
//!   `usize`-based so the ABI is correct on both), `string` as a
//!   NUL-terminated `char*` (kept alive for the duration of the call).
//! - The return value is really consumed: `int`/`bool`/`char` from the
//!   integer return register, `float` from the XMM0 return register,
//!   `string` by copying the returned `char*` (the callee keeps ownership;
//!   the bridge never frees foreign memory).
//!
//! ## Honest limits (rejected at type-check time, enforced again here)
//!
//! - `float` *parameters* would require per-signature XMM marshalling and
//!   are not supported yet — only `float` returns.
//! - `bytes` parameters lose their length at the ABI boundary (pointer
//!   only), so they are rejected; `bytes` returns are rejected.
//! - At most 8 parameters (fixed-arity prototypes below).
//! - The first time a library is loaded its handle is kept for the whole
//!   process lifetime and never closed (avoids use-after-free of cached
//!   symbols). This is documented behaviour, not a leak by accident.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::mem::transmute;
use std::os::raw::{c_char, c_void};
use std::sync::Mutex;

use titan_codegen::{BytecodeType, ExternDecl};

use crate::{Value, VmError};

#[cfg(unix)]
const RTLD_LAZY: i32 = 1;

#[cfg(unix)]
extern "C" {
    fn dlopen(filename: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *mut c_char;
}

// GetProcAddress returns FARPROC (a function pointer); we read it as a
// pointer-sized integer, which is the same representation on every
// supported Windows target.
#[cfg(windows)]
extern "system" {
    fn LoadLibraryA(lp_file_name: *const c_char) -> *mut c_void;
    fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const c_char) -> usize;
}

/// Candidate C libraries, in order. `libc.so.6` is the versioned name on
/// glibc/musl and on Android API 26+; older Android only ships `libc.so`;
/// `libdl` historically held dlopen itself on glibc < 2.34.
pub fn default_libraries() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["libSystem.dylib", "libc.dylib"]
    } else if cfg!(windows) {
        &["msvcrt.dll", "ucrtbase.dll"]
    } else if cfg!(target_os = "android") {
        &["libc.so.6", "libc.so"]
    } else {
        &["libc.so.6", "libc.so", "libdl.so.2", "libdl.so"]
    }
}

#[cfg(unix)]
fn last_dl_error() -> String {
    unsafe {
        let message = dlerror();
        if message.is_null() {
            "unknown dynamic linker error".into()
        } else {
            CStr::from_ptr(message).to_string_lossy().into_owned()
        }
    }
}

#[cfg(unix)]
fn load_library(path: &str) -> Result<usize, String> {
    let c_path = CString::new(path).map_err(|_| format!("library path '{path}' contains a NUL byte"))?;
    let handle = unsafe { dlopen(c_path.as_ptr(), RTLD_LAZY) };
    if handle.is_null() {
        Err(last_dl_error())
    } else {
        Ok(handle as usize)
    }
}

#[cfg(windows)]
fn load_library(path: &str) -> Result<usize, String> {
    let c_path = CString::new(path).map_err(|_| format!("library path '{path}' contains a NUL byte"))?;
    let handle = unsafe { LoadLibraryA(c_path.as_ptr()) };
    if handle.is_null() {
        Err("LoadLibraryA failed".into())
    } else {
        Ok(handle as usize)
    }
}

#[cfg(unix)]
fn find_symbol(handle: usize, symbol: &str) -> Result<usize, String> {
    let c_symbol = CString::new(symbol).map_err(|_| format!("symbol '{symbol}' contains a NUL byte"))?;
    let address = unsafe { dlsym(handle as *mut c_void, c_symbol.as_ptr()) };
    if address.is_null() {
        Err(last_dl_error())
    } else {
        Ok(address as usize)
    }
}

#[cfg(windows)]
fn find_symbol(handle: usize, symbol: &str) -> Result<usize, String> {
    let c_symbol = CString::new(symbol).map_err(|_| format!("symbol '{symbol}' contains a NUL byte"))?;
    let address = unsafe { GetProcAddress(handle as *mut c_void, c_symbol.as_ptr()) };
    if address == 0 {
        Err("GetProcAddress returned null".into())
    } else {
        Ok(address)
    }
}

/// Resolve `symbol` in `library`, caching both the loaded handle and the
/// resolved address process-wide. The handle is deliberately never closed.
pub fn resolve(
    libraries: &Mutex<HashMap<String, usize>>,
    symbols: &Mutex<HashMap<String, usize>>,
    library: &str,
    symbol: &str,
) -> Result<usize, VmError> {
    let key = format!("{library}\u{1}{symbol}");
    if let Some(address) = symbols
        .lock()
        .map_err(|_| VmError::Type("extern symbol registry poisoned".into()))?
        .get(&key)
        .copied()
    {
        return Ok(address);
    }

    let mut libraries_guard = libraries
        .lock()
        .map_err(|_| VmError::Type("extern library registry poisoned".into()))?;
    let handle = match libraries_guard.get(library).copied() {
        Some(handle) => handle,
        None => {
            let loaded = load_library(library).map_err(|message| VmError::Native {
                function: format!("extern '{symbol}'"),
                message: format!("cannot load library '{library}': {message}"),
            })?;
            libraries_guard.insert(library.to_string(), loaded);
            loaded
        }
    };

    let address = find_symbol(handle, symbol).map_err(|message| VmError::Native {
        function: format!("extern '{symbol}'"),
        message: format!("symbol '{symbol}' not found in '{library}': {message}"),
    })?;
    drop(libraries_guard);
    symbols
        .lock()
        .map_err(|_| VmError::Type("extern symbol registry poisoned".into()))?
        .insert(key, address);
    Ok(address)
}

/// Marshals `args` (already arity-checked) into 8-byte ABI units.
/// `String` arguments are kept alive in the returned `Vec<CString>` until
/// the foreign call has returned.
fn marshal(decl: &ExternDecl, args: Vec<Value>) -> Result<(Vec<usize>, Vec<CString>), VmError> {
    let mut units: Vec<usize> = Vec::with_capacity(args.len());
    let mut keep_alive: Vec<CString> = Vec::new();
    for (index, (expected, argument)) in decl.param_types.iter().zip(args).enumerate() {
        let unit = match (expected, argument) {
            (BytecodeType::Int, Value::Int(value)) => value as usize,
            (BytecodeType::Bool, Value::Bool(value)) => usize::from(value),
            (BytecodeType::Char, Value::Char(value)) => value as u32 as usize,
            (BytecodeType::String, Value::Str(value)) => {
                let c_string = CString::new(value).map_err(|_| VmError::Type(format!(
                    "extern '{}' parameter {index} is a string containing a NUL byte", decl.name
                )))?;
                let pointer = c_string.as_ptr() as usize;
                keep_alive.push(c_string);
                pointer
            }
            (BytecodeType::Float, _) => return Err(VmError::Type(format!(
                "extern '{}' parameter {index}: float parameters are not supported by the C ABI bridge yet (use int, bool, char or string)", decl.name
            ))),
            (BytecodeType::Bytes, _) => return Err(VmError::Type(format!(
                "extern '{}' parameter {index}: bytes parameters lose their length at the C ABI boundary and are not supported", decl.name
            ))),
            (expected, found) => return Err(VmError::Type(format!(
                "extern '{}' parameter {index}: expected {expected:?}, found {found:?}", decl.name
            ))),
        };
        units.push(unit);
    }
    Ok((units, keep_alive))
}

/// Calls the resolved symbol with a fixed-arity prototype. `ret_float`
/// selects the XMM-returning prototype (for `-> float`); everything else
/// reads the integer return register.
fn invoke(address: usize, units: &[usize], ret_float: bool) -> Result<u64, VmError> {
    if units.len() > 8 {
        return Err(VmError::Type("extern calls support at most 8 parameters".into()));
    }
    unsafe {
        let ptr = address as *const c_void;
        if ret_float {
            let bits: u64 = match units.len() {
                0 => { let f: unsafe extern "C" fn() -> f64 = transmute(ptr); f().to_bits() }
                1 => { let f: unsafe extern "C" fn(usize) -> f64 = transmute(ptr); f(units[0]).to_bits() }
                2 => { let f: unsafe extern "C" fn(usize, usize) -> f64 = transmute(ptr); f(units[0], units[1]).to_bits() }
                3 => { let f: unsafe extern "C" fn(usize, usize, usize) -> f64 = transmute(ptr); f(units[0], units[1], units[2]).to_bits() }
                4 => { let f: unsafe extern "C" fn(usize, usize, usize, usize) -> f64 = transmute(ptr); f(units[0], units[1], units[2], units[3]).to_bits() }
                5 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize) -> f64 = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4]).to_bits() }
                6 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize, usize) -> f64 = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4], units[5]).to_bits() }
                7 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize, usize, usize) -> f64 = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4], units[5], units[6]).to_bits() }
                8 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize, usize, usize, usize) -> f64 = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4], units[5], units[6], units[7]).to_bits() }
                _ => unreachable!(),
            };
            Ok(bits)
        } else {
            let result: usize = match units.len() {
                0 => { let f: unsafe extern "C" fn() -> usize = transmute(ptr); f() }
                1 => { let f: unsafe extern "C" fn(usize) -> usize = transmute(ptr); f(units[0]) }
                2 => { let f: unsafe extern "C" fn(usize, usize) -> usize = transmute(ptr); f(units[0], units[1]) }
                3 => { let f: unsafe extern "C" fn(usize, usize, usize) -> usize = transmute(ptr); f(units[0], units[1], units[2]) }
                4 => { let f: unsafe extern "C" fn(usize, usize, usize, usize) -> usize = transmute(ptr); f(units[0], units[1], units[2], units[3]) }
                5 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize) -> usize = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4]) }
                6 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize, usize) -> usize = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4], units[5]) }
                7 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize, usize, usize) -> usize = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4], units[5], units[6]) }
                8 => { let f: unsafe extern "C" fn(usize, usize, usize, usize, usize, usize, usize, usize) -> usize = transmute(ptr); f(units[0], units[1], units[2], units[3], units[4], units[5], units[6], units[7]) }
                _ => unreachable!(),
            };
            Ok(result as u64)
        }
    }
}

/// Converts a machine-word return value to an `i64` preserving the C
/// function's signedness on 32-bit targets (int-class returns are 32-bit
/// there, so sign-extend; on 64-bit targets the word is already 64-bit).
fn decode_int(result: usize) -> i64 {
    if cfg!(target_pointer_width = "64") {
        result as i64
    } else {
        (result as i32) as i64
    }
}

/// Full bridge: resolve → marshal → call → decode. When the extern does not

/// name a library, each platform C-library candidate is tried in order.
pub fn call(
    decl: &ExternDecl,
    args: Vec<Value>,
    libraries: &Mutex<HashMap<String, usize>>,
    symbols: &Mutex<HashMap<String, usize>>,
) -> Result<Value, VmError> {
    if args.len() != decl.param_types.len() {
        return Err(VmError::Arity { function: decl.name.clone(), expected: decl.param_types.len(), found: args.len() });
    }
    let address = if let Some(library) = &decl.library {
        resolve(libraries, symbols, library, &decl.name)?
    } else {
        let mut last_error: Option<VmError> = None;
        let mut found = None;
        for library in default_libraries() {
            match resolve(libraries, symbols, library, &decl.name) {
                Ok(address) => { found = Some(address); break; }
                Err(error) => last_error = Some(error),
            }
        }
        found.ok_or_else(|| last_error.unwrap_or_else(|| VmError::Type("no C library is available on this platform".into())))?
    };
    let (units, _keep_alive) = marshal(decl, args)?;
    let ret_float = matches!(decl.return_type, Some(BytecodeType::Float));
    let result = invoke(address, &units, ret_float)?;

    Ok(match decl.return_type.as_ref() {
        None | Some(BytecodeType::Unknown) => Value::Nil,
        Some(BytecodeType::Int) => Value::Int(decode_int(result as usize)),
        Some(BytecodeType::Bool) => Value::Bool(result != 0),
        Some(BytecodeType::Char) => Value::Char(char::from_u32(result as u32).unwrap_or('\0')),
        Some(BytecodeType::Float) => Value::Float(f64::from_bits(result)),
        Some(BytecodeType::String) => {
            let pointer = result as *const c_char;
            if pointer.is_null() {
                return Err(VmError::Type(format!("extern '{}' returned a NULL string", decl.name)));
            }
            // Copy the string. The callee keeps ownership of its buffer;
            // the bridge never calls free() on foreign memory.
            let text = unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned();
            Value::Str(text)
        }
        Some(other) => return Err(VmError::Type(format!(
            "extern '{}' has unsupported return type {other:?}", decl.name
        ))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extern_decl(name: &str, params: Vec<BytecodeType>, ret: Option<BytecodeType>) -> ExternDecl {
        ExternDecl { name: name.into(), library: None, param_types: params, return_type: ret }
    }

    fn bridge(decl: &ExternDecl, args: Vec<Value>) -> Result<Value, VmError> {
        call(decl, args, &Mutex::new(HashMap::new()), &Mutex::new(HashMap::new()))
    }

    #[test]
    fn strlen_from_libc_is_real() {
        let decl = extern_decl("strlen", vec![BytecodeType::String], Some(BytecodeType::Int));
        assert_eq!(bridge(&decl, vec![Value::Str("titan".into())]).unwrap(), Value::Int(5));
    }

    #[test]
    fn getpid_from_libc_is_real() {
        let decl = extern_decl("getpid", vec![], Some(BytecodeType::Int));
        match bridge(&decl, vec![]).unwrap() {
            Value::Int(pid) => assert!(pid > 0),
            other => panic!("expected pid int, got {other:?}"),
        }
    }

    #[test]
    fn missing_symbol_is_a_typed_error_not_a_noop() {
        let decl = extern_decl("titan_symbol_that_does_not_exist_xyz", vec![], Some(BytecodeType::Int));
        assert!(matches!(bridge(&decl, vec![]), Err(VmError::Native { .. })));
    }
}
