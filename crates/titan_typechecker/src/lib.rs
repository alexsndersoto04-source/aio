//! Semantic analysis and type checking for Titan.

use std::collections::{HashMap, HashSet};
use thiserror::Error;
use titan_ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    Char,
    Nil,
    Unit,
    Never,
    Array(Box<Type>),
    Tuple(Vec<Type>),
    Named(String),
    Function(Vec<Type>, Box<Type>),
    Unknown,
}

// Internal evidence that an inferred collection contains incompatible concrete
// element types. It behaves as dynamic data when the expected contract is
// `any`, but unlike Unknown it cannot silently satisfy `[int]` or `[string]`.
const MIXED_ELEMENT_TYPE: &str = "$titan::mixed-element";

#[derive(Debug, Clone)]
enum MatchDomain {
    Empty,
    Bool,
    Enum {
        name: String,
        variants: HashSet<String>,
    },
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MatchAtom {
    Bool(bool),
    EnumVariant(String),
    Int(i64),
    String(String),
    Char(char),
    Nil,
}

#[derive(Debug, Clone, Default)]
struct PatternCoverage {
    all: bool,
    atoms: HashSet<MatchAtom>,
}

#[derive(Debug, Clone, Default)]
struct MatchCoverage {
    all: bool,
    atoms: HashSet<MatchAtom>,
}

impl MatchCoverage {
    fn is_complete(&self, domain: &MatchDomain) -> bool {
        if self.all {
            return true;
        }
        match domain {
            MatchDomain::Empty => true,
            MatchDomain::Bool => {
                self.atoms.contains(&MatchAtom::Bool(false))
                    && self.atoms.contains(&MatchAtom::Bool(true))
            }
            MatchDomain::Enum { variants, .. } => variants.iter().all(|variant| {
                self.atoms
                    .contains(&MatchAtom::EnumVariant(variant.clone()))
            }),
            MatchDomain::Open => false,
        }
    }

    fn covers(&self, pattern: &PatternCoverage, domain: &MatchDomain) -> bool {
        if self.all {
            return true;
        }
        if pattern.all {
            return self.is_complete(domain);
        }
        !pattern.atoms.is_empty() && pattern.atoms.is_subset(&self.atoms)
    }

    fn add(&mut self, pattern: PatternCoverage) {
        self.all |= pattern.all;
        self.atoms.extend(pattern.atoms);
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if matches!(self, Type::Named(name) if name == MIXED_ELEMENT_TYPE) {
            f.write_str("mixed")
        } else {
            write!(f, "{:?}", self)
        }
    }
}

#[derive(Error, Debug, Clone, PartialEq)]
pub enum TypeError {
    #[error("type mismatch: expected {expected}, found {found}")]
    Mismatch { expected: Type, found: Type },
    #[error("unknown variable or function '{name}'")]
    UnknownVariable { name: String },
    #[error("'{name}' is not callable")]
    NotCallable { name: String },
    #[error("function expected {expected} arguments, found {found}")]
    Arity { expected: usize, found: usize },
    #[error("invalid operand for {operator}: {operand}")]
    InvalidUnary { operator: String, operand: Type },
    #[error("invalid operands for {operator}: {left} and {right}")]
    InvalidOperands {
        operator: String,
        left: Type,
        right: Type,
    },
    #[error("value of type {target} cannot be indexed")]
    NotIndexable { target: Type },
    #[error("index {index} is out of bounds for a tuple of length {length}")]
    IndexOutOfBounds { index: i64, length: usize },
    #[error("value of type {target} is not iterable")]
    NotIterable { target: Type },
    #[error("type {receiver} has no method '{method}'")]
    UnknownMethod { receiver: Type, method: String },
    #[error("assignment target must be a mutable local variable in the current function")]
    InvalidAssignmentTarget,
    #[error("return used outside a function or closure")]
    OutsideFunction,
    #[error("function '{name}' can finish without returning {expected}")]
    MissingReturn { name: String, expected: Type },
    #[error("function '{name}' returns incompatible types {first} and {second}")]
    InconsistentReturns {
        name: String,
        first: Type,
        second: Type,
    },
    #[error("duplicate {kind} declaration '{name}'")]
    DuplicateDeclaration { kind: String, name: String },
    #[error("unknown type '{name}'")]
    UnknownType { name: String },
    #[error("type '{name}' expects {expected} type arguments, found {found}")]
    InvalidTypeArguments {
        name: String,
        expected: usize,
        found: usize,
    },
    #[error("recursive constant dependency: {cycle}")]
    RecursiveConstant { cycle: String },
    #[error("type alias '{name}' is recursive")]
    RecursiveTypeAlias { name: String },
    #[error("impl target '{name}' is not a declared struct")]
    InvalidImplTarget { name: String },
    #[error("method '{method}' in impl {trait_name} for {target} has an incompatible signature")]
    TraitMethodMismatch {
        trait_name: String,
        target: String,
        method: String,
    },
    #[error("method '{method}' is not declared by trait '{trait_name}'")]
    UnknownTraitMethod { trait_name: String, method: String },
    #[error("missing field '{field}' in struct '{structure}'")]
    MissingField { structure: String, field: String },
    #[error("unknown field '{field}' in struct '{structure}'")]
    UnknownField { structure: String, field: String },
    #[error("field '{field}' appears more than once in struct literal '{structure}'")]
    DuplicateField { structure: String, field: String },
    #[error("invalid argument to '{function}': expected {expected}, found {found}")]
    InvalidArgument {
        function: String,
        expected: String,
        found: Type,
    },
    #[error("invalid string interpolation expression '{expression}'")]
    InvalidInterpolation { expression: String },
    #[error("value of type {target} has no fields")]
    NoFields { target: Type },
    #[error("invalid pattern: {message}")]
    InvalidPattern { message: String },
    #[error("unknown variant '{enumeration}::{variant}'")]
    UnknownVariant {
        enumeration: String,
        variant: String,
    },
    #[error("non-exhaustive match; add a catch-all arm")]
    NonExhaustiveMatch,
    #[error("non-exhaustive match for enum '{enumeration}'; missing {missing:?}")]
    NonExhaustiveEnum {
        enumeration: String,
        missing: Vec<String>,
    },
    #[error("match arm {arm} has an unreachable pattern")]
    UnreachablePattern { arm: usize },
    #[error("break/continue used outside a loop")]
    OutsideLoop,
    #[error("operator ? requires an Option or Result value")]
    InvalidTry,
    #[error("unsupported language feature: {feature}")]
    UnsupportedFeature { feature: String },
}

#[derive(Clone)]
struct FunctionSig {
    params: Vec<Type>,
    result: Type,
}

pub struct TypeEnv {
    scopes: Vec<HashMap<String, Type>>,
    bindings: Vec<HashMap<String, (bool, usize)>>,
    functions: HashMap<String, FunctionSig>,
    base_functions: HashMap<String, FunctionSig>,
    structs: HashMap<String, HashMap<String, Type>>,
    enum_variants: HashMap<String, Option<Type>>,
    base_enum_variants: HashMap<String, Option<Type>>,
    constants: HashSet<String>,
    constant_types: HashMap<String, Type>,
    constant_declarations: HashMap<String, ConstDecl>,
    constant_stack: Vec<String>,
    checked_constants: HashSet<String>,
    /// Phase 22: trait declarations indexed by name, so `impl Trait for
    /// Type` blocks can pull default method bodies + signatures.
    traits: HashMap<String, TraitDecl>,
    /// Phase 28: `type Alias = Existing` map. Resolved lazily in
    /// `resolve_alias` after `type_from_ast` produces a Type::Named that
    /// doesn't match any built-in or user-declared struct/enum.
    type_aliases: HashMap<String, Type>,
    errors: Vec<TypeError>,
    return_type: Type,
    return_candidates: Vec<Vec<Type>>,
    loop_depth: usize,
    loop_breaks: Vec<bool>,
    function_depth: usize,
}

impl TypeEnv {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        functions.insert(
            "print".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Nil,
            },
        );
        functions.insert(
            "println".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Nil,
            },
        );
        functions.insert(
            "len".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "map".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown],
                result: Type::Array(Box::new(Type::Unknown)),
            },
        );
        functions.insert(
            "filter".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown],
                result: Type::Array(Box::new(Type::Unknown)),
            },
        );
        functions.insert(
            "fold".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown, Type::Unknown],
                result: Type::Unknown,
            },
        );
        // Phase 19: sort_by(arr, |a,b| cmp) -> arr;  find/any/all(arr, |x| bool)
        functions.insert(
            "sort_by".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown],
                result: Type::Array(Box::new(Type::Unknown)),
            },
        );
        functions.insert(
            "find".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown],
                result: Type::Unknown,
            },
        );
        functions.insert(
            "any".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown],
                result: Type::Bool,
            },
        );
        functions.insert(
            "all".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown],
                result: Type::Bool,
            },
        );
        functions.insert(
            "join".into(),
            FunctionSig {
                params: vec![Type::Named("Task".into())],
                result: Type::Unknown,
            },
        );
        functions.insert(
            "join_timeout".into(),
            FunctionSig {
                params: vec![Type::Named("Task".into()), Type::Int],
                result: Type::Named("Option".into()),
            },
        );
        functions.insert(
            "cancel".into(),
            FunctionSig {
                params: vec![Type::Named("Task".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "channel".into(),
            FunctionSig {
                params: vec![Type::Int],
                result: Type::Tuple(vec![
                    Type::Named("Sender".into()),
                    Type::Named("Receiver".into()),
                ]),
            },
        );
        functions.insert(
            "send".into(),
            FunctionSig {
                params: vec![Type::Named("Sender".into()), Type::Unknown],
                result: Type::Nil,
            },
        );
        functions.insert(
            "recv".into(),
            FunctionSig {
                params: vec![Type::Named("Receiver".into())],
                result: Type::Unknown,
            },
        );
        functions.insert(
            "recv_timeout".into(),
            FunctionSig {
                params: vec![Type::Named("Receiver".into()), Type::Int],
                result: Type::Named("Option".into()),
            },
        );
        functions.insert(
            "select".into(),
            FunctionSig {
                params: vec![
                    Type::Array(Box::new(Type::Named("Receiver".into()))),
                    Type::Int,
                ],
                result: Type::Named("Option".into()),
            },
        );
        functions.insert(
            "std::net::tcp_listen".into(),
            FunctionSig {
                params: vec![Type::String],
                result: Type::Named("TcpListener".into()),
            },
        );
        functions.insert(
            "std::net::tcp_local_addr".into(),
            FunctionSig {
                params: vec![Type::Named("TcpListener".into())],
                result: Type::String,
            },
        );
        functions.insert(
            "std::net::tcp_accept".into(),
            FunctionSig {
                params: vec![Type::Named("TcpListener".into())],
                result: Type::Tuple(vec![Type::Named("TcpStream".into()), Type::String]),
            },
        );
        functions.insert(
            "std::net::tcp_connect".into(),
            FunctionSig {
                params: vec![Type::String],
                result: Type::Named("TcpStream".into()),
            },
        );
        functions.insert(
            "std::net::tcp_read".into(),
            FunctionSig {
                params: vec![Type::Named("TcpStream".into()), Type::Int],
                result: Type::Named("bytes".into()),
            },
        );
        functions.insert(
            "std::net::tcp_write".into(),
            FunctionSig {
                params: vec![Type::Named("TcpStream".into()), Type::Named("bytes".into())],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::net::tcp_set_timeout".into(),
            FunctionSig {
                params: vec![Type::Named("TcpStream".into()), Type::Int],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::net::tcp_close".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::http::serve_connection".into(),
            FunctionSig {
                params: vec![
                    Type::Named("TcpListener".into()),
                    Type::Function(
                        vec![Type::Named("map".into())],
                        Box::new(Type::Named("map".into())),
                    ),
                    Type::Int,
                ],
                result: Type::String,
            },
        );
        functions.insert(
            "std::http::router".into(),
            FunctionSig {
                params: vec![],
                result: Type::Named("HttpRouter".into()),
            },
        );
        functions.insert(
            "std::http::route".into(),
            FunctionSig {
                params: vec![
                    Type::Named("HttpRouter".into()),
                    Type::String,
                    Type::String,
                    Type::Function(vec![Type::Named("map".into())], Box::new(Type::Unknown)),
                ],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::http::middleware".into(),
            FunctionSig {
                params: vec![
                    Type::Named("HttpRouter".into()),
                    Type::Function(
                        vec![Type::Named("map".into())],
                        Box::new(Type::Named("map".into())),
                    ),
                ],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::http::after".into(),
            FunctionSig {
                params: vec![
                    Type::Named("HttpRouter".into()),
                    Type::Function(
                        vec![Type::Named("map".into())],
                        Box::new(Type::Named("map".into())),
                    ),
                ],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::http::on_error".into(),
            FunctionSig {
                params: vec![
                    Type::Named("HttpRouter".into()),
                    Type::Function(
                        vec![Type::Named("map".into()), Type::Named("map".into())],
                        Box::new(Type::Named("map".into())),
                    ),
                ],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::http::dispatch".into(),
            FunctionSig {
                params: vec![Type::Named("HttpRouter".into()), Type::Named("map".into())],
                result: Type::Unknown,
            },
        );
        functions.insert(
            "std::tls::connect".into(),
            FunctionSig {
                params: vec![Type::String, Type::String],
                result: Type::Named("TlsStream".into()),
            },
        );
        functions.insert(
            "std::tls::server_config".into(),
            FunctionSig {
                params: vec![Type::String, Type::String],
                result: Type::Named("TlsServerConfig".into()),
            },
        );
        functions.insert(
            "std::tls::accept".into(),
            FunctionSig {
                params: vec![
                    Type::Named("TcpListener".into()),
                    Type::Named("TlsServerConfig".into()),
                ],
                result: Type::Tuple(vec![Type::Named("TlsStream".into()), Type::String]),
            },
        );
        functions.insert(
            "std::tls::read".into(),
            FunctionSig {
                params: vec![Type::Named("TlsStream".into()), Type::Int],
                result: Type::Named("bytes".into()),
            },
        );
        functions.insert(
            "std::tls::write".into(),
            FunctionSig {
                params: vec![Type::Named("TlsStream".into()), Type::Named("bytes".into())],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::tls::close".into(),
            FunctionSig {
                params: vec![Type::Named("TlsStream".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::ws::decoder".into(),
            FunctionSig {
                params: vec![Type::Int],
                result: Type::Named("WebSocketDecoder".into()),
            },
        );
        functions.insert(
            "std::ws::decoder_push".into(),
            FunctionSig {
                params: vec![
                    Type::Named("WebSocketDecoder".into()),
                    Type::Named("bytes".into()),
                ],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::ws::decoder_next".into(),
            FunctionSig {
                params: vec![Type::Named("WebSocketDecoder".into()), Type::Bool],
                result: Type::Named("Option".into()),
            },
        );
        functions.insert(
            "std::ws::connect".into(),
            FunctionSig {
                params: vec![Type::String, Type::String, Type::Int],
                result: Type::Named("WebSocket".into()),
            },
        );
        functions.insert(
            "std::ws::attach_tcp".into(),
            FunctionSig {
                params: vec![Type::Named("TcpStream".into()), Type::Bool, Type::Int],
                result: Type::Named("WebSocket".into()),
            },
        );
        functions.insert(
            "std::ws::attach_tls".into(),
            FunctionSig {
                params: vec![Type::Named("TlsStream".into()), Type::Bool, Type::Int],
                result: Type::Named("WebSocket".into()),
            },
        );
        functions.insert(
            "std::ws::send_text".into(),
            FunctionSig {
                params: vec![Type::Named("WebSocket".into()), Type::String],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::ws::send_binary".into(),
            FunctionSig {
                params: vec![Type::Named("WebSocket".into()), Type::Named("bytes".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::ws::receive".into(),
            FunctionSig {
                params: vec![Type::Named("WebSocket".into())],
                result: Type::Unknown,
            },
        );
        functions.insert(
            "std::ws::close".into(),
            FunctionSig {
                params: vec![Type::Named("WebSocket".into()), Type::Int, Type::String],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::server::control".into(),
            FunctionSig {
                params: vec![Type::Int],
                result: Type::Named("ServerControl".into()),
            },
        );
        functions.insert(
            "std::server::try_acquire".into(),
            FunctionSig {
                params: vec![Type::Named("ServerControl".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::server::release".into(),
            FunctionSig {
                params: vec![Type::Named("ServerControl".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::server::shutdown".into(),
            FunctionSig {
                params: vec![Type::Named("ServerControl".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::server::stats".into(),
            FunctionSig {
                params: vec![Type::Named("ServerControl".into())],
                result: Type::Named("map".into()),
            },
        );
        functions.insert(
            "std::server::health_response".into(),
            FunctionSig {
                params: vec![Type::Named("ServerControl".into())],
                result: Type::Named("map".into()),
            },
        );
        functions.insert(
            "std::sqlite::open".into(),
            FunctionSig {
                params: vec![Type::String],
                result: Type::Named("Sqlite".into()),
            },
        );
        functions.insert(
            "std::sqlite::memory".into(),
            FunctionSig {
                params: vec![],
                result: Type::Named("Sqlite".into()),
            },
        );
        functions.insert(
            "std::sqlite::execute".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into()), Type::String, Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::sqlite::query".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into()), Type::String, Type::Unknown],
                result: Type::Array(Box::new(Type::Named("map".into()))),
            },
        );
        functions.insert(
            "std::sqlite::begin".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::sqlite::commit".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::sqlite::rollback".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::sqlite::migrate".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into()), Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::sqlite::last_insert_id".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into())],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::sqlite::close".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::sqlite::ping".into(),
            FunctionSig {
                params: vec![Type::Named("Sqlite".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::sqlite::pool".into(),
            FunctionSig {
                params: vec![Type::String, Type::Int],
                result: Type::Named("SqlitePool".into()),
            },
        );
        functions.insert(
            "std::sqlite::acquire".into(),
            FunctionSig {
                params: vec![Type::Named("SqlitePool".into()), Type::Int],
                result: Type::Named("Option".into()),
            },
        );
        functions.insert(
            "std::sqlite::pool_stats".into(),
            FunctionSig {
                params: vec![Type::Named("SqlitePool".into())],
                result: Type::Named("map".into()),
            },
        );
        functions.insert(
            "std::sqlite::pool_health".into(),
            FunctionSig {
                params: vec![Type::Named("SqlitePool".into()), Type::Int],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::sqlite::pool_close".into(),
            FunctionSig {
                params: vec![Type::Named("SqlitePool".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::postgres::connect".into(),
            FunctionSig {
                params: vec![Type::String],
                result: Type::Named("Postgres".into()),
            },
        );
        functions.insert(
            "std::postgres::connect_tls".into(),
            FunctionSig {
                params: vec![Type::String],
                result: Type::Named("Postgres".into()),
            },
        );
        functions.insert(
            "std::postgres::execute".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into()), Type::String, Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::postgres::query".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into()), Type::String, Type::Unknown],
                result: Type::Array(Box::new(Type::Named("map".into()))),
            },
        );
        functions.insert(
            "std::postgres::begin".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::postgres::commit".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::postgres::rollback".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::postgres::cancel".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::postgres::migrate".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into()), Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::postgres::close".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::postgres::pool".into(),
            FunctionSig {
                params: vec![Type::String, Type::Int, Type::Bool],
                result: Type::Named("PostgresPool".into()),
            },
        );
        functions.insert(
            "std::postgres::acquire".into(),
            FunctionSig {
                params: vec![Type::Named("PostgresPool".into()), Type::Int],
                result: Type::Named("Option".into()),
            },
        );
        functions.insert(
            "std::postgres::pool_stats".into(),
            FunctionSig {
                params: vec![Type::Named("PostgresPool".into())],
                result: Type::Named("map".into()),
            },
        );
        functions.insert(
            "std::postgres::ping".into(),
            FunctionSig {
                params: vec![Type::Named("Postgres".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::postgres::pool_health".into(),
            FunctionSig {
                params: vec![Type::Named("PostgresPool".into()), Type::Int],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::postgres::pool_close".into(),
            FunctionSig {
                params: vec![Type::Named("PostgresPool".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::mysql::connect".into(),
            FunctionSig {
                params: vec![Type::String],
                result: Type::Named("Mysql".into()),
            },
        );
        functions.insert(
            "std::mysql::execute".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into()), Type::String, Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::mysql::query".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into()), Type::String, Type::Unknown],
                result: Type::Array(Box::new(Type::Named("map".into()))),
            },
        );
        functions.insert(
            "std::mysql::begin".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::mysql::commit".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::mysql::rollback".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::mysql::migrate".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into()), Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::mysql::last_insert_id".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into())],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::mysql::close".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::mysql::ping".into(),
            FunctionSig {
                params: vec![Type::Named("Mysql".into())],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::mysql::pool".into(),
            FunctionSig {
                params: vec![Type::String, Type::Int],
                result: Type::Named("MysqlPool".into()),
            },
        );
        functions.insert(
            "std::mysql::acquire".into(),
            FunctionSig {
                params: vec![Type::Named("MysqlPool".into()), Type::Int],
                result: Type::Named("Option".into()),
            },
        );
        functions.insert(
            "std::mysql::pool_stats".into(),
            FunctionSig {
                params: vec![Type::Named("MysqlPool".into())],
                result: Type::Named("map".into()),
            },
        );
        functions.insert(
            "std::mysql::pool_health".into(),
            FunctionSig {
                params: vec![Type::Named("MysqlPool".into()), Type::Int],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::mysql::pool_close".into(),
            FunctionSig {
                params: vec![Type::Named("MysqlPool".into())],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::db::execute".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::String, Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::db::query".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::String, Type::Unknown],
                result: Type::Array(Box::new(Type::Named("map".into()))),
            },
        );
        functions.insert(
            "std::db::begin".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::db::commit".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::db::rollback".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::db::migrate".into(),
            FunctionSig {
                params: vec![Type::Unknown, Type::Unknown],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::db::close".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::db::ping".into(),
            FunctionSig {
                params: vec![Type::Unknown],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::runtime::memory_limit".into(),
            FunctionSig {
                params: vec![],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::runtime::allocated_bytes".into(),
            FunctionSig {
                params: vec![],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::runtime::gc_live_count".into(),
            FunctionSig {
                params: vec![],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::runtime::gc_collect".into(),
            FunctionSig {
                params: vec![],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::runtime::gc_threshold".into(),
            FunctionSig {
                params: vec![],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::runtime::gc_set_threshold".into(),
            FunctionSig {
                params: vec![Type::Int],
                result: Type::Nil,
            },
        );
        functions.insert(
            "std::runtime::active_tasks".into(),
            FunctionSig {
                params: vec![],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::runtime::heap_dump".into(),
            FunctionSig {
                params: vec![Type::String],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::runtime::optimize_level".into(),
            FunctionSig {
                params: vec![],
                result: Type::Int,
            },
        );
        functions.insert(
            "std::runtime::fast_path_enabled".into(),
            FunctionSig {
                params: vec![],
                result: Type::Bool,
            },
        );
        functions.insert(
            "std::runtime::benchmark".into(),
            FunctionSig {
                params: vec![
                    Type::Int,
                    Type::Function(Vec::new(), Box::new(Type::Unknown)),
                ],
                result: Type::Named("map".into()),
            },
        );
        functions.insert(
            "std::runtime::spawn_quota".into(),
            FunctionSig {
                params: vec![
                    Type::Int,
                    Type::Function(Vec::new(), Box::new(Type::Unknown)),
                ],
                result: Type::Named("Task".into()),
            },
        );
        let enum_variants = HashMap::from([
            ("Option::None".into(), None),
            ("Option::Some".into(), Some(Type::Unknown)),
            ("Result::Ok".into(), Some(Type::Unknown)),
            ("Result::Err".into(), Some(Type::Unknown)),
        ]);
        Self {
            scopes: vec![HashMap::new()],
            bindings: vec![HashMap::new()],
            base_functions: functions.clone(),
            functions,
            structs: HashMap::new(),
            base_enum_variants: enum_variants.clone(),
            enum_variants,
            constants: HashSet::new(),
            constant_types: HashMap::new(),
            constant_declarations: HashMap::new(),
            constant_stack: Vec::new(),
            checked_constants: HashSet::new(),
            traits: HashMap::new(),
            type_aliases: HashMap::new(),
            errors: Vec::new(),
            return_type: Type::Unknown,
            return_candidates: Vec::new(),
            loop_depth: 0,
            loop_breaks: Vec::new(),
            function_depth: 0,
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), Vec<TypeError>> {
        // A TypeEnv can be reused by the LSP and other embedders. User
        // declarations and local scopes from a previous program must never
        // leak into the next one; only the built-in registry is persistent.
        self.scopes = vec![HashMap::new()];
        self.bindings = vec![HashMap::new()];
        self.functions = self.base_functions.clone();
        self.structs.clear();
        self.enum_variants = self.base_enum_variants.clone();
        self.constants.clear();
        self.constant_types.clear();
        self.constant_declarations.clear();
        self.constant_stack.clear();
        self.checked_constants.clear();
        self.traits.clear();
        self.type_aliases.clear();
        self.errors.clear();
        self.return_type = Type::Unknown;
        self.return_candidates.clear();
        self.loop_depth = 0;
        self.loop_breaks.clear();
        self.function_depth = 0;
        self.validate_declarations(&program.items);
        self.collect_declarations(&program.items);
        self.validate_type_aliases();
        self.validate_declared_types(&program.items);
        let mut validation_errors = std::mem::take(&mut self.errors);

        // Unannotated function returns and global constants are inferred
        // together to a fixed point before the diagnostic pass. Both are
        // predeclared, so their types do not depend on source order and a
        // constant may safely refer to another constant declared later.
        for _ in 0..=count_inference_targets(&program.items) {
            let previous_functions: HashMap<_, _> = self
                .functions
                .iter()
                .map(|(name, signature)| (name.clone(), signature.result.clone()))
                .collect();
            let previous_constants = self.constant_types.clone();
            self.reset_analysis_state();
            for item in &program.items {
                self.check_item(item);
            }
            let functions_stable = self.functions.iter().all(|(name, signature)| {
                previous_functions
                    .get(name)
                    .is_some_and(|result| result == &signature.result)
            });
            if functions_stable && previous_constants == self.constant_types {
                break;
            }
        }

        // Trait contracts must use the inferred method results, not the
        // initial Unknown placeholders collected from unannotated methods.
        self.errors.clear();
        self.validate_impl_contracts(&program.items);
        validation_errors.append(&mut self.errors);

        self.reset_analysis_state();
        self.errors = validation_errors;
        for item in &program.items {
            self.check_item(item);
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn reset_analysis_state(&mut self) {
        self.scopes = vec![self.constant_types.clone()];
        self.bindings = vec![self
            .constant_types
            .keys()
            .map(|name| (name.clone(), (false, 0)))
            .collect()];
        self.constants = self.constant_types.keys().cloned().collect();
        self.constant_stack.clear();
        self.checked_constants.clear();
        self.errors.clear();
        self.return_type = Type::Unknown;
        self.return_candidates.clear();
        self.loop_depth = 0;
        self.loop_breaks.clear();
        self.function_depth = 0;
    }

    fn infer_return_type(&mut self, name: &str, candidates: &[Type]) -> Type {
        let Some(first) = candidates.first() else {
            return Type::Unit;
        };
        let first_resolved = self.resolve_alias(first);
        for candidate in candidates.iter().skip(1) {
            let resolved = self.resolve_alias(candidate);
            if !compatible(&first_resolved, &resolved) && !compatible(&resolved, &first_resolved) {
                self.errors.push(TypeError::InconsistentReturns {
                    name: name.into(),
                    first: first.clone(),
                    second: candidate.clone(),
                });
                return Type::Unknown;
            }
        }
        first.clone()
    }

    fn validate_declarations(&mut self, items: &[Item]) {
        self.errors
            .extend(declaration_errors(&self.base_functions, items));
    }

    fn validate_type_aliases(&mut self) {
        for (name, target) in &self.type_aliases {
            let mut visited = HashSet::new();
            if alias_reaches(name, target, &self.type_aliases, &mut visited) {
                self.errors
                    .push(TypeError::RecursiveTypeAlias { name: name.clone() });
            }
        }
    }

    fn validate_declared_types(&mut self, items: &[Item]) {
        let mut known = builtin_type_names(&self.base_functions);
        collect_declared_type_names(items, &mut known);
        self.errors.extend(declared_type_errors(items, &known));
    }

    fn validate_impl_contracts(&mut self, items: &[Item]) {
        let mut methods = HashSet::new();
        self.validate_impl_contracts_in(items, &mut methods);
    }

    fn validate_impl_contracts_in(&mut self, items: &[Item], methods: &mut HashSet<String>) {
        for item in items {
            match item {
                Item::Impl(block) => {
                    let Some(target) = direct_named_type(&block.target_type) else {
                        self.errors.push(TypeError::InvalidImplTarget {
                            name: format!("{:?}", block.target_type),
                        });
                        continue;
                    };
                    if !self.structs.contains_key(target) {
                        self.errors.push(TypeError::InvalidImplTarget {
                            name: target.into(),
                        });
                    }
                    let provided: HashMap<_, _> = block
                        .methods
                        .iter()
                        .map(|method| (method.name.as_str(), method))
                        .collect();
                    for method in &block.methods {
                        let qualified = format!("{}::{}", target, method.name);
                        if !methods.insert(qualified.clone()) {
                            self.errors.push(TypeError::DuplicateDeclaration {
                                kind: "method".into(),
                                name: qualified,
                            });
                        }
                    }
                    let Some(trait_name) = &block.trait_name else {
                        continue;
                    };
                    let Some(trait_decl) = self.traits.get(trait_name).cloned() else {
                        continue;
                    };
                    let trait_methods: HashMap<_, _> = trait_decl
                        .methods
                        .iter()
                        .map(|method| (method.name.as_str(), method))
                        .collect();
                    for method in &block.methods {
                        let Some(contract) = trait_methods.get(method.name.as_str()) else {
                            self.errors.push(TypeError::UnknownTraitMethod {
                                trait_name: trait_name.clone(),
                                method: method.name.clone(),
                            });
                            continue;
                        };
                        let expected = trait_method_signature(contract, target);
                        let qualified = format!("{}::{}", target, method.name);
                        let found = self
                            .functions
                            .get(&qualified)
                            .cloned()
                            .unwrap_or_else(|| function_signature(method, Some(target)));
                        if !self.signatures_equal(&expected, &found) {
                            self.errors.push(TypeError::TraitMethodMismatch {
                                trait_name: trait_name.clone(),
                                target: target.into(),
                                method: method.name.clone(),
                            });
                        }
                    }
                    for method in &trait_decl.methods {
                        if provided.contains_key(method.name.as_str()) || method.body.is_none() {
                            continue;
                        }
                        let qualified = format!("{}::{}", target, method.name);
                        if !methods.insert(qualified.clone()) {
                            self.errors.push(TypeError::DuplicateDeclaration {
                                kind: "method".into(),
                                name: qualified,
                            });
                        }
                    }
                }
                Item::Module(module) => self.validate_impl_contracts_in(&module.items, methods),
                _ => {}
            }
        }
    }

    fn signatures_equal(&self, expected: &FunctionSig, found: &FunctionSig) -> bool {
        expected.params.len() == found.params.len()
            && expected
                .params
                .iter()
                .zip(&found.params)
                .all(|(left, right)| self.resolve_alias(left) == self.resolve_alias(right))
            && self.resolve_alias(&expected.result) == self.resolve_alias(&found.result)
    }

    fn collect_traits(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Trait(t) => {
                    self.traits.insert(t.name.clone(), t.clone());
                }
                Item::Module(m) => self.collect_traits(&m.items),
                _ => {}
            }
        }
    }

    /// Phase 28: pre-collect `type Alias = Existing` before anything else
    /// so aliases can appear in ANY signature (including fns defined
    /// earlier than the alias) without file-order issues. The target is
    /// stored raw as Type; expansion happens lazily via resolve_alias
    /// during compatibility checks and field/method lookups.
    fn collect_type_aliases(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::TypeAlias(t) => {
                    let target = type_from_ast(&t.target);
                    self.type_aliases.insert(t.name.clone(), target);
                }
                Item::Module(m) => self.collect_type_aliases(&m.items),
                _ => {}
            }
        }
    }

    fn collect_declarations(&mut self, items: &[Item]) {
        // Phase 28: first pass — type aliases so signatures can reference them.
        self.collect_type_aliases(items);
        // Phase 22: second pass — traits, so `impl Trait for T` blocks
        // can look up default bodies regardless of file order.
        self.collect_traits(items);
        for item in items {
            match item {
                Item::Function(function) => {
                    self.functions.insert(
                        function.name.clone(),
                        FunctionSig {
                            params: function
                                .params
                                .iter()
                                .map(|p| {
                                    p.type_ann
                                        .as_ref()
                                        .map(type_from_ast)
                                        .unwrap_or(Type::Unknown)
                                })
                                .collect(),
                            result: function
                                .return_type
                                .as_ref()
                                .map(type_from_ast)
                                .unwrap_or(Type::Unknown),
                        },
                    );
                }
                Item::Struct(structure) => {
                    self.structs.insert(
                        structure.name.clone(),
                        structure
                            .fields
                            .iter()
                            .map(|f| (f.name.clone(), type_from_ast(&f.type_ann)))
                            .collect(),
                    );
                }
                Item::Enum(enumeration) => {
                    for variant in &enumeration.variants {
                        self.enum_variants.insert(
                            format!("{}::{}", enumeration.name, variant.name),
                            variant.payload.as_ref().map(type_from_ast),
                        );
                    }
                }
                Item::Const(constant) => {
                    let declared = constant
                        .type_ann
                        .as_ref()
                        .map(type_from_ast)
                        .unwrap_or(Type::Unknown);
                    self.constant_types.insert(constant.name.clone(), declared);
                    self.constant_declarations
                        .insert(constant.name.clone(), constant.clone());
                }
                Item::Module(module) => self.collect_declarations(&module.items),
                // Phase 20: register `impl Type { fn m() {} }` methods
                // under qualified names `Type::m` so different structs
                // can share method names without colliding. When the
                // first parameter is named `self` and lacks an
                // annotation, synthesize it as Type::Named(type_name)
                // so field access on `self` typechecks correctly.
                //
                // Phase 22: for `impl Trait for Type { ... }` we also
                // inherit signatures (and eventually bodies via codegen)
                // for every trait method with a default body that this
                // impl doesn't override.
                Item::Impl(block) => {
                    let type_name = match &block.target_type {
                        TypeExpr::Named { name, .. } => Some(name.clone()),
                        _ => None,
                    };
                    let mut provided: HashSet<String> = HashSet::new();
                    for method in &block.methods {
                        provided.insert(method.name.clone());
                        let qualified = match &type_name {
                            Some(t) => format!("{}::{}", t, method.name),
                            None => method.name.clone(),
                        };
                        let params: Vec<Type> = method
                            .params
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                if i == 0 && p.name == "self" && p.type_ann.is_none() {
                                    if let Some(t) = &type_name {
                                        return Type::Named(t.clone());
                                    }
                                }
                                p.type_ann
                                    .as_ref()
                                    .map(type_from_ast)
                                    .unwrap_or(Type::Unknown)
                            })
                            .collect();
                        self.functions.insert(
                            qualified,
                            FunctionSig {
                                params,
                                result: method
                                    .return_type
                                    .as_ref()
                                    .map(type_from_ast)
                                    .unwrap_or(Type::Unit),
                            },
                        );
                    }
                    // Phase 22: inherit default-method signatures from
                    // the trait, and report a UnknownVariable error if a
                    // required method (no default body) is missing.
                    if let (Some(trait_name), Some(type_name)) = (&block.trait_name, &type_name) {
                        if let Some(trait_decl) = self.traits.get(trait_name).cloned() {
                            for tm in &trait_decl.methods {
                                if provided.contains(&tm.name) {
                                    continue;
                                }
                                let qualified = format!("{}::{}", type_name, tm.name);
                                let params: Vec<Type> = tm
                                    .params
                                    .iter()
                                    .enumerate()
                                    .map(|(i, p)| {
                                        if i == 0 && p.name == "self" && p.type_ann.is_none() {
                                            return Type::Named(type_name.clone());
                                        }
                                        p.type_ann
                                            .as_ref()
                                            .map(type_from_ast)
                                            .unwrap_or(Type::Unknown)
                                    })
                                    .collect();
                                if tm.body.is_some() {
                                    // Default provided by trait — register signature so calls typecheck.
                                    self.functions.insert(
                                        qualified,
                                        FunctionSig {
                                            params,
                                            result: tm
                                                .return_type
                                                .as_ref()
                                                .map(type_from_ast)
                                                .unwrap_or(Type::Unit),
                                        },
                                    );
                                } else {
                                    // Required method missing.
                                    self.errors.push(TypeError::UnknownVariable {
                                        name: format!(
                                            "impl {} for {}: missing required method '{}'",
                                            trait_name, type_name, tm.name
                                        ),
                                    });
                                }
                            }
                        } else {
                            self.errors.push(TypeError::UnknownVariable {
                                name: format!("trait '{}'", trait_name),
                            });
                        }
                    }
                }
                Item::Trait(trait_decl) => {
                    // Phase 22: record so impls can look up defaults later.
                    self.traits
                        .insert(trait_decl.name.clone(), trait_decl.clone());
                }
                _ => {}
            }
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(function) => self.check_function(function),
            Item::Impl(block) => {
                // Phase 20: when checking an impl method's body, `self`
                // (if present) must resolve as the impl's target type,
                // not Unknown. Synthesize the annotation on-the-fly in
                // a cloned FunctionDecl so field/method access on `self`
                // typechecks against the struct's schema.
                let type_name = match &block.target_type {
                    TypeExpr::Named { name, .. } => Some(name.clone()),
                    _ => None,
                };
                let provided: HashSet<String> =
                    block.methods.iter().map(|m| m.name.clone()).collect();
                for method in &block.methods {
                    let mut annotated = method.clone();
                    if let Some(t) = &type_name {
                        annotated.name = format!("{}::{}", t, method.name);
                    }
                    if let (Some(t), Some(first)) = (&type_name, annotated.params.first_mut()) {
                        if first.name == "self" && first.type_ann.is_none() {
                            first.type_ann = Some(TypeExpr::Named {
                                name: t.clone(),
                                generics: Vec::new(),
                            });
                        }
                    }
                    self.check_function(&annotated);
                }
                // Phase 22: also typecheck inherited defaults, treating
                // them as if declared on this type — catches errors
                // early when a default body references self.field but
                // the impl target doesn't have such a field, etc.
                if let (Some(trait_name), Some(t)) = (&block.trait_name, &type_name) {
                    if let Some(trait_decl) = self.traits.get(trait_name).cloned() {
                        for tm in &trait_decl.methods {
                            if provided.contains(&tm.name) {
                                continue;
                            }
                            let Some(body) = tm.body.clone() else {
                                continue;
                            };
                            let synth = FunctionDecl {
                                name: format!("{}::{}", t, tm.name),
                                source_file: None,
                                params: tm
                                    .params
                                    .iter()
                                    .enumerate()
                                    .map(|(i, p)| {
                                        let mut cloned = p.clone();
                                        if i == 0
                                            && cloned.name == "self"
                                            && cloned.type_ann.is_none()
                                        {
                                            cloned.type_ann = Some(TypeExpr::Named {
                                                name: t.clone(),
                                                generics: Vec::new(),
                                            });
                                        }
                                        cloned
                                    })
                                    .collect(),
                                return_type: tm.return_type.clone(),
                                body: Some(body),
                                is_extern: false,
                                abi: None,
                                span: tm.span,
                            };
                            self.check_function(&synth);
                        }
                    }
                }
            }
            Item::Module(module) => {
                for item in &module.items {
                    self.check_item(item);
                }
            }
            Item::Const(constant) => {
                self.check_constant(&constant.name);
            }
            _ => {}
        }
    }

    fn check_constant(&mut self, name: &str) -> Type {
        if self.checked_constants.contains(name) {
            return self
                .constant_types
                .get(name)
                .cloned()
                .unwrap_or(Type::Unknown);
        }
        if let Some(start) = self
            .constant_stack
            .iter()
            .position(|constant| constant == name)
        {
            let mut cycle = self.constant_stack[start..].to_vec();
            cycle.push(name.into());
            self.errors.push(TypeError::RecursiveConstant {
                cycle: cycle.join(" -> "),
            });
            return Type::Unknown;
        }
        let Some(constant) = self.constant_declarations.get(name).cloned() else {
            return Type::Unknown;
        };
        self.constant_stack.push(name.into());
        let found = self.check_expr(&constant.value);
        self.constant_stack.pop();
        let declared = constant.type_ann.as_ref().map(type_from_ast);
        if let Some(expected) = &declared {
            self.require_compatible(expected, &found);
        }
        let inferred = declared.unwrap_or(found);
        self.constant_types.insert(name.into(), inferred.clone());
        self.scopes[0].insert(name.into(), inferred.clone());
        self.bindings[0].insert(name.into(), (false, 0));
        self.checked_constants.insert(name.into());
        inferred
    }

    fn check_identifier(&mut self, name: &str) -> Type {
        let locally_bound = self
            .scopes
            .iter()
            .skip(1)
            .rev()
            .any(|scope| scope.contains_key(name));
        if !locally_bound && self.constant_declarations.contains_key(name) {
            return self.check_constant(name);
        }
        if let Some(ty) = self.lookup(name) {
            return ty;
        }
        // Dedicated built-ins only exist as direct-call bytecode operations;
        // codegen cannot materialize them as closure values. Rejecting such a
        // reference here prevents the checker from accepting code that later
        // fails with UnknownVariable during lowering. Direct calls bypass this
        // value path in check_call and retain their normal signatures.
        if self.base_functions.contains_key(name) {
            self.errors.push(TypeError::UnsupportedFeature {
                feature: format!("built-in function values ('{name}')"),
            });
            return Type::Unknown;
        }
        self.functions
            .get(name)
            .map(|signature| {
                Type::Function(signature.params.clone(), Box::new(signature.result.clone()))
            })
            .or_else(|| {
                self.enum_variants.get(name).and_then(|payload| {
                    if payload.is_none() {
                        name.split_once("::")
                            .map(|(enumeration, _)| Type::Named(enumeration.into()))
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| {
                self.errors
                    .push(TypeError::UnknownVariable { name: name.into() });
                Type::Unknown
            })
    }

    fn check_function(&mut self, function: &FunctionDecl) {
        self.push_scope();
        for param in &function.params {
            let param_type = param
                .type_ann
                .as_ref()
                .map(type_from_ast)
                .unwrap_or(Type::Unknown);
            if let Some(default) = &param.default {
                let found = self.check_expr(default);
                self.require_compatible(&param_type, &found);
            }
            // A default may refer to earlier parameters, but a parameter is
            // not in scope inside its own default expression.
            self.define_at_depth(
                param.name.clone(),
                param_type,
                param.mutable,
                self.function_depth + 1,
            );
        }
        let old_return = std::mem::replace(
            &mut self.return_type,
            function
                .return_type
                .as_ref()
                .map(type_from_ast)
                .unwrap_or(Type::Unknown),
        );
        let old_loop_depth = std::mem::replace(&mut self.loop_depth, 0);
        let old_loop_breaks = std::mem::take(&mut self.loop_breaks);
        self.function_depth += 1;
        self.return_candidates.push(Vec::new());
        if let Some(body) = &function.body {
            // Feed an explicit return contract into a trailing closure before
            // checking its body. A post-hoc Unknown-compatible comparison is
            // too late to validate omitted closure parameter annotations.
            let body_type = if function.return_type.is_some() {
                self.check_block_expected(body, Some(&self.return_type.clone()))
            } else {
                self.check_block(body)
            };
            if function.return_type.is_none() {
                if body.final_expr.is_some() {
                    if body_type != Type::Never {
                        if let Some(candidates) = self.return_candidates.last_mut() {
                            candidates.push(body_type.clone());
                        }
                    }
                } else if body_type != Type::Never && !block_definitely_returns(body) {
                    if let Some(candidates) = self.return_candidates.last_mut() {
                        candidates.push(Type::Unit);
                    }
                }
                if body_type == Type::Never {
                    if let Some(candidates) = self.return_candidates.last_mut() {
                        if candidates.is_empty() {
                            candidates.push(Type::Never);
                        }
                    }
                }
            }
            if function.return_type.is_some()
                && !compatible(&self.return_type, &Type::Unit)
                && body.final_expr.is_none()
                && body_type != Type::Never
                && !block_definitely_returns(body)
            {
                self.errors.push(TypeError::MissingReturn {
                    name: function.name.clone(),
                    expected: self.return_type.clone(),
                });
            }
        }
        let candidates = self.return_candidates.pop().unwrap_or_default();
        if function.return_type.is_none() && function.body.is_some() {
            let inferred = self.infer_return_type(&function.name, &candidates);
            if let Some(signature) = self.functions.get_mut(&function.name) {
                signature.result = inferred;
            }
        }
        self.function_depth -= 1;
        self.loop_depth = old_loop_depth;
        self.loop_breaks = old_loop_breaks;
        self.return_type = old_return;
        self.pop_scope();
    }

    fn check_block(&mut self, block: &Block) -> Type {
        self.check_block_expected(block, None)
    }

    fn check_block_expected(&mut self, block: &Block, expected: Option<&Type>) -> Type {
        self.push_scope();
        let mut diverges = false;
        for stmt in &block.stmts {
            let was_diverged = diverges;
            let candidate_count = self.return_candidates.last().map(Vec::len);
            let loop_broke = self.loop_breaks.last().copied();
            let statement_type = match stmt {
                Stmt::Let {
                    name,
                    mutable,
                    type_ann,
                    value,
                    ..
                } => {
                    let (binding_type, value_type) = if let Some(annotation) = type_ann {
                        let binding_type = type_from_ast(annotation);
                        let value_type = self.check_expr_expected(value, &binding_type);
                        (binding_type, value_type)
                    } else {
                        let value_type = self.check_expr(value);
                        (value_type.clone(), value_type)
                    };
                    self.define_mutable(name.clone(), binding_type, *mutable);
                    value_type
                }
                Stmt::Assign {
                    target, op, value, ..
                } => self.check_assignment(target, *op, value),
                Stmt::Expr(expr) => self.check_expr(expr),
                Stmt::Item(_) => {
                    self.errors.push(TypeError::UnsupportedFeature {
                        feature: "nested declarations".into(),
                    });
                    Type::Unit
                }
            };
            if was_diverged {
                if let (Some(candidates), Some(count)) =
                    (self.return_candidates.last_mut(), candidate_count)
                {
                    candidates.truncate(count);
                }
                if let (Some(current), Some(previous)) = (self.loop_breaks.last_mut(), loop_broke) {
                    *current = previous;
                }
            }
            diverges |= statement_type == Type::Never;
        }

        let candidate_count = self.return_candidates.last().map(Vec::len);
        let loop_broke = self.loop_breaks.last().copied();
        let result = block
            .final_expr
            .as_ref()
            .map(|expression| {
                if let Some(expected) = expected {
                    self.check_expr_expected(expression, expected)
                } else {
                    self.check_expr(expression)
                }
            })
            .unwrap_or(Type::Unit);
        if diverges {
            if let (Some(candidates), Some(count)) =
                (self.return_candidates.last_mut(), candidate_count)
            {
                candidates.truncate(count);
            }
            if let (Some(current), Some(previous)) = (self.loop_breaks.last_mut(), loop_broke) {
                *current = previous;
            }
        }
        self.pop_scope();
        if diverges {
            Type::Never
        } else {
            result
        }
    }

    fn restore_control_flow(
        &mut self,
        candidate_count: Option<usize>,
        loop_broke: Option<bool>,
    ) {
        if let (Some(candidates), Some(count)) =
            (self.return_candidates.last_mut(), candidate_count)
        {
            candidates.truncate(count);
        }
        if let (Some(current), Some(previous)) = (self.loop_breaks.last_mut(), loop_broke) {
            *current = previous;
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Int { .. } => Type::Int,
            Expr::Float { .. } => Type::Float,
            Expr::String { .. } => Type::String,
            Expr::StringTemplate { value, .. } => self.check_string_template(value),
            Expr::Char { .. } => Type::Char,
            Expr::Bool { .. } => Type::Bool,
            Expr::Nil { .. } => Type::Nil,
            Expr::Ident { name, .. } => {
                let ty = self.check_identifier(name);
                if self.resolve_alias(&ty) == Type::Never {
                    Type::Never
                } else {
                    ty
                }
            }
            Expr::Array { elements, .. } => {
                // Heterogeneous arrays remain valid runtime values, but retain
                // evidence that their element type is mixed. Widening them to
                // Unknown lost that evidence and allowed `[1, "text"]` to
                // satisfy a later `[int]` contract through an inferred local.
                // Phase 28: resolve aliases per-element so a literal like
                // [Named("Tag"), ...] (where `type Tag = string`) unifies
                // to Array(String) — otherwise a struct field typed as
                // `tags: [string]` receives `Array(Named("Tag"))` and the
                // require_compatible check against the field type fails
                // deep inside the container (compatible doesn't hop aliases).
                let mut reachable = true;
                let types: Vec<Type> = elements
                    .iter()
                    .map(|element| {
                        let ty = self.check_evaluated_expr(element, None, &mut reachable);
                        self.resolve_alias(&ty)
                    })
                    .collect();
                if !reachable {
                    Type::Never
                } else {
                    let inner = match types.first() {
                        None => Type::Unknown,
                        Some(head) => {
                            if types.iter().skip(1).all(|t| compatible(head, t)) {
                                head.clone()
                            } else {
                                Type::Named(MIXED_ELEMENT_TYPE.into())
                            }
                        }
                    };
                    Type::Array(Box::new(inner))
                }
            }
            Expr::Tuple { elements, .. } => {
                let mut reachable = true;
                let types: Vec<_> = elements
                    .iter()
                    .map(|element| self.check_evaluated_expr(element, None, &mut reachable))
                    .collect();
                if !reachable {
                    Type::Never
                } else {
                    Type::Tuple(types)
                }
            }
            Expr::StructLit { name, fields, .. } => {
                let mut reachable = true;
                if let Some(expected_fields) = self.structs.get(name).cloned() {
                    let mut supplied = HashSet::new();
                    for (field, value) in fields {
                        if !supplied.insert(field.as_str()) {
                            self.errors.push(TypeError::DuplicateField {
                                structure: name.clone(),
                                field: field.clone(),
                            });
                        }
                        if let Some(expected) = expected_fields.get(field) {
                            self.check_evaluated_expr(value, Some(expected), &mut reachable);
                        } else {
                            self.check_evaluated_expr(value, None, &mut reachable);
                            self.errors.push(TypeError::UnknownField {
                                structure: name.clone(),
                                field: field.clone(),
                            });
                        }
                    }
                    for field in expected_fields.keys() {
                        if !supplied.contains(field.as_str()) {
                            self.errors.push(TypeError::MissingField {
                                structure: name.clone(),
                                field: field.clone(),
                            });
                        }
                    }
                } else {
                    self.errors
                        .push(TypeError::UnknownType { name: name.clone() });
                    for (_, value) in fields {
                        self.check_evaluated_expr(value, None, &mut reachable);
                    }
                }
                if !reachable {
                    Type::Never
                } else {
                    Type::Named(name.clone())
                }
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let mut reachable = true;
                let left_type = self.check_evaluated_expr(left, None, &mut reachable);
                let lazy_right = if lazy_rhs_is_skipped(*op, left) {
                    Some(false)
                } else if lazy_rhs_is_guaranteed(*op, left) {
                    Some(true)
                } else {
                    None
                };
                // A compile-time short-circuit still has its unreachable side
                // typechecked, but returns and breaks there cannot affect the
                // enclosing function or loop. Conversely, when a boolean
                // literal guarantees RHS evaluation, divergence must propagate
                // exactly as it does for an eager operator.
                let right_type = if lazy_right == Some(false) {
                    let mut right_reachable = false;
                    self.check_evaluated_expr(right, None, &mut right_reachable)
                } else {
                    self.check_evaluated_expr(right, None, &mut reachable)
                };
                // Phase 28: normalize both sides through type aliases
                // so `Score >= int` (where `type Score = int`) works.
                let left_type = self.resolve_alias(&left_type);
                let right_type = self.resolve_alias(&right_type);
                let lazy = matches!(op, BinaryOp::LazyAnd | BinaryOp::LazyOr);
                if left_type == Type::Never
                    || (right_type == Type::Never && (!lazy || lazy_right == Some(true)))
                {
                    Type::Never
                } else {
                    self.check_binary(*op, left_type, right_type)
                }
            }
            Expr::Range { start, end, .. } => {
                let mut reachable = true;
                let a = self.check_evaluated_expr(start, None, &mut reachable);
                let b = self.check_evaluated_expr(end, None, &mut reachable);
                self.require_compatible(&Type::Int, &a);
                self.require_compatible(&Type::Int, &b);
                if self.resolve_alias(&a) == Type::Never || self.resolve_alias(&b) == Type::Never {
                    Type::Never
                } else {
                    Type::Array(Box::new(Type::Int))
                }
            }
            Expr::Unary { op, expr, .. } => {
                let raw = self.check_expr(expr);
                let ty = self.resolve_alias(&raw);
                if ty == Type::Never {
                    return Type::Never;
                }
                match op {
                    UnaryOp::Not => {
                        self.require_compatible(&Type::Bool, &ty);
                        Type::Bool
                    }
                    UnaryOp::Neg => {
                        if !is_numeric(&ty) {
                            self.errors.push(TypeError::InvalidUnary {
                                operator: "Neg".into(),
                                operand: ty.clone(),
                            });
                            Type::Unknown
                        } else {
                            ty
                        }
                    }
                    UnaryOp::BitNot => {
                        if !compatible(&Type::Int, &ty) {
                            self.errors.push(TypeError::InvalidUnary {
                                operator: "BitNot".into(),
                                operand: ty,
                            });
                            Type::Unknown
                        } else {
                            Type::Int
                        }
                    }
                    UnaryOp::Ref | UnaryOp::RefMut | UnaryOp::Deref => {
                        self.errors.push(TypeError::UnsupportedFeature {
                            feature: "references and dereferencing".into(),
                        });
                        Type::Unknown
                    }
                }
            }
            Expr::Call { callee, args, .. } => {
                let result = self.check_call(callee, args);
                if self.resolve_alias(&result) == Type::Never
                    || expr_definitely_returns(callee)
                    || args.iter().any(expr_definitely_returns)
                {
                    Type::Never
                } else {
                    result
                }
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let result = self.check_method_call(receiver, method, args);
                if self.resolve_alias(&result) == Type::Never
                    || expr_definitely_returns(receiver)
                    || args.iter().any(expr_definitely_returns)
                {
                    Type::Never
                } else {
                    result
                }
            }
            Expr::Index { target, index, .. } => {
                let mut reachable = true;
                let target_raw = self.check_evaluated_expr(target, None, &mut reachable);
                let target_type = self.resolve_alias(&target_raw);
                let index_type = self.check_evaluated_expr(index, None, &mut reachable);
                if target_type == Type::Never || index_type == Type::Never {
                    return Type::Never;
                }
                match target_type {
                    Type::Array(inner) => {
                        self.require_compatible(&Type::Int, &index_type);
                        *inner
                    }
                    Type::Tuple(items) => {
                        self.require_compatible(&Type::Int, &index_type);
                        if let Expr::Int { value, .. } = index.as_ref() {
                            if let Some(found) = usize::try_from(*value)
                                .ok()
                                .and_then(|position| items.get(position).cloned())
                            {
                                found
                            } else {
                                self.errors.push(TypeError::IndexOutOfBounds {
                                    index: *value,
                                    length: items.len(),
                                });
                                Type::Unknown
                            }
                        } else {
                            common_type(&items).unwrap_or(Type::Unknown)
                        }
                    }
                    Type::String => {
                        self.require_compatible(&Type::Int, &index_type);
                        Type::Char
                    }
                    Type::Named(name) if name == "bytes" => {
                        self.require_compatible(&Type::Int, &index_type);
                        Type::Int
                    }
                    Type::Named(name) if name == "map" => {
                        self.require_compatible(&Type::String, &index_type);
                        Type::Unknown
                    }
                    Type::Unknown => Type::Unknown,
                    target => {
                        self.errors.push(TypeError::NotIndexable {
                            target: target.clone(),
                        });
                        Type::Unknown
                    }
                }
            }
            Expr::FieldAccess { target, field, .. } => {
                let raw = self.check_expr(target);
                match self.resolve_alias(&raw) {
                    Type::Never => Type::Never,
                    Type::Named(name) if name == "map" => Type::Unknown,
                    Type::Named(name) => self
                        .structs
                        .get(&name)
                        .and_then(|s| s.get(field))
                        .cloned()
                        .unwrap_or_else(|| {
                            self.errors.push(TypeError::UnknownField {
                                structure: name,
                                field: field.clone(),
                            });
                            Type::Unknown
                        }),
                    Type::Unknown => Type::Unknown,
                    target => {
                        self.errors.push(TypeError::NoFields {
                            target: target.clone(),
                        });
                        Type::Unknown
                    }
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_type = self.check_expr(condition);
                self.require_compatible(&Type::Bool, &condition_type);
                let literal_condition = match condition.as_ref() {
                    Expr::Bool { value, .. } => Some(*value),
                    _ => None,
                };
                let candidate_count = self.return_candidates.last().map(Vec::len);
                let loop_broke = self.loop_breaks.last().copied();
                let then_type = self.check_block(then_branch);
                let then_candidate_count = self.return_candidates.last().map(Vec::len);
                let then_loop_broke = self.loop_breaks.last().copied();

                // The then branch is unreachable when the condition cannot
                // finish or is literally false. Keep its diagnostics, but do
                // not let impossible returns or breaks escape the branch.
                if condition_type == Type::Never || literal_condition == Some(false) {
                    self.restore_control_flow(candidate_count, loop_broke);
                }

                if let Some(other) = else_branch {
                    let else_type = self.check_block(other);
                    if condition_type == Type::Never {
                        self.restore_control_flow(candidate_count, loop_broke);
                        Type::Never
                    } else if literal_condition == Some(true) {
                        // The else branch is statically unreachable. Restore
                        // the effects produced by the reachable then branch.
                        self.restore_control_flow(then_candidate_count, then_loop_broke);
                        then_type
                    } else if literal_condition == Some(false) {
                        else_type
                    } else if then_type == Type::Never {
                        else_type
                    } else if else_type == Type::Never {
                        then_type
                    } else if compatible(
                        &self.resolve_alias(&then_type),
                        &self.resolve_alias(&else_type),
                    ) {
                        then_type
                    } else {
                        self.errors.push(TypeError::Mismatch {
                            expected: then_type,
                            found: else_type,
                        });
                        Type::Unknown
                    }
                } else if condition_type == Type::Never {
                    Type::Never
                } else if literal_condition == Some(true) && then_type == Type::Never {
                    Type::Never
                } else {
                    Type::Unit
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let subject = self.check_expr(scrutinee);
                self.check_match_coverage(&subject, arms);
                let subject_never = self.resolve_alias(&subject) == Type::Never;
                let domain = self.match_domain(&subject);
                let known_subject = self.known_match_atom(scrutinee, &domain);
                let candidate_count = self.return_candidates.last().map(Vec::len);
                let loop_broke = self.loop_breaks.last().copied();
                let mut known_path_open = known_subject.is_some();
                let mut result: Option<Type> = None;
                for arm in arms {
                    if !match_pattern_is_lowerable(&arm.pattern) {
                        self.errors.push(TypeError::UnsupportedFeature {
                            feature: "or-patterns and nested destructuring in match".into(),
                        });
                    }
                    let arm_candidate_count = self.return_candidates.last().map(Vec::len);
                    let arm_loop_broke = self.loop_breaks.last().copied();
                    let arm_reachable = !subject_never
                        && known_subject.as_ref().map_or(true, |atom| {
                            known_path_open
                                && self
                                    .pattern_coverage(&arm.pattern, &domain)
                                    .is_some_and(|pattern| {
                                        pattern.all || pattern.atoms.contains(atom)
                                    })
                        });

                    self.push_scope();
                    self.bind_pattern(&arm.pattern, &subject);
                    let (guard_never, guard_literal) = if let Some(guard) = &arm.guard {
                        let guard_type = self.check_expr(guard);
                        self.require_compatible(&Type::Bool, &guard_type);
                        let literal = match guard.as_ref() {
                            Expr::Bool { value, .. } => Some(*value),
                            _ => None,
                        };
                        (guard_type == Type::Never, literal)
                    } else {
                        (false, None)
                    };
                    let body_candidate_count = self.return_candidates.last().map(Vec::len);
                    let body_loop_broke = self.loop_breaks.last().copied();
                    let body_type = self.check_block(&arm.body);
                    let body_reachable = arm_reachable
                        && !guard_never
                        && guard_literal != Some(false);
                    if !arm_reachable {
                        self.restore_control_flow(arm_candidate_count, arm_loop_broke);
                    } else if !body_reachable {
                        // The guard is evaluated on this path, but a false or
                        // diverging guard prevents its body from running.
                        self.restore_control_flow(body_candidate_count, body_loop_broke);
                    }

                    let found = if !arm_reachable || guard_literal == Some(false) {
                        None
                    } else if guard_never {
                        Some(Type::Never)
                    } else {
                        Some(body_type)
                    };
                    if let Some(found) = found {
                        result = Some(match result {
                            None => found,
                            Some(current) if current == Type::Never => found,
                            Some(current) if found == Type::Never => current,
                            Some(current)
                                if compatible(
                                    &self.resolve_alias(&current),
                                    &self.resolve_alias(&found),
                                ) =>
                            {
                                current
                            }
                            Some(current) => {
                                self.errors.push(TypeError::Mismatch {
                                    expected: current,
                                    found,
                                });
                                Type::Unknown
                            }
                        });
                    }

                    if known_subject.is_some() && arm_reachable {
                        if guard_never || arm.guard.is_none() || guard_literal == Some(true) {
                            known_path_open = false;
                        }
                    }
                    self.pop_scope();
                }
                if subject_never {
                    self.restore_control_flow(candidate_count, loop_broke);
                    Type::Never
                } else {
                    result.unwrap_or(Type::Unknown)
                }
            }
            Expr::For {
                pattern,
                iterator,
                body,
                ..
            } => {
                if !matches!(pattern.as_ref(), Pattern::Ident { .. }) {
                    self.errors.push(TypeError::UnsupportedFeature {
                        feature: "destructuring patterns in for loops".into(),
                    });
                }
                let raw = self.check_expr(iterator);
                let iterator_type = self.resolve_alias(&raw);
                let item = match &iterator_type {
                    Type::Never => Type::Never,
                    Type::Array(inner) => *inner.clone(),
                    Type::Tuple(items) => common_type(items).unwrap_or(Type::Unknown),
                    Type::String => Type::Char,
                    Type::Named(name) if name == "bytes" => Type::Int,
                    Type::Unknown => Type::Unknown,
                    _ => {
                        self.errors.push(TypeError::NotIterable {
                            target: iterator_type.clone(),
                        });
                        Type::Unknown
                    }
                };
                self.push_scope();
                self.bind_pattern(pattern, &item);
                self.loop_depth += 1;
                self.loop_breaks.push(false);
                let candidate_count = self.return_candidates.last().map(Vec::len);
                self.check_block(body);
                self.loop_breaks.pop();
                self.loop_depth -= 1;
                self.pop_scope();
                if iterator_type == Type::Never {
                    if let (Some(candidates), Some(count)) =
                        (self.return_candidates.last_mut(), candidate_count)
                    {
                        candidates.truncate(count);
                    }
                    Type::Never
                } else {
                    Type::Unit
                }
            }
            Expr::While {
                condition, body, ..
            } => {
                let c = self.check_expr(condition);
                self.require_compatible(&Type::Bool, &c);
                self.loop_depth += 1;
                self.loop_breaks.push(false);
                let candidate_count = self.return_candidates.last().map(Vec::len);
                self.check_block(body);
                let body_may_break = self.loop_breaks.pop().unwrap_or(false);
                self.loop_depth -= 1;
                let literal_condition = match condition.as_ref() {
                    Expr::Bool { value, .. } => Some(*value),
                    _ => None,
                };
                if c == Type::Never || literal_condition == Some(false) {
                    if let (Some(candidates), Some(count)) =
                        (self.return_candidates.last_mut(), candidate_count)
                    {
                        candidates.truncate(count);
                    }
                }
                if c == Type::Never {
                    Type::Never
                } else if literal_condition == Some(true) && !body_may_break {
                    Type::Never
                } else {
                    Type::Unit
                }
            }
            Expr::Loop { body, .. } => {
                self.loop_depth += 1;
                self.loop_breaks.push(false);
                self.check_block(body);
                let body_may_break = self.loop_breaks.pop().unwrap_or(false);
                self.loop_depth -= 1;
                if body_may_break {
                    Type::Unit
                } else {
                    Type::Never
                }
            }
            Expr::Break { value, .. } => {
                let reaches_break = if let Some(value) = value {
                    let found = self.check_expr(value);
                    self.errors.push(TypeError::UnsupportedFeature {
                        feature: "values carried by break".into(),
                    });
                    self.resolve_alias(&found) != Type::Never
                } else {
                    true
                };
                if self.loop_depth == 0 {
                    self.errors.push(TypeError::OutsideLoop);
                } else if reaches_break {
                    if let Some(current) = self.loop_breaks.last_mut() {
                        *current = true;
                    }
                }
                Type::Never
            }
            Expr::Continue { .. } => {
                if self.loop_depth == 0 {
                    self.errors.push(TypeError::OutsideLoop);
                }
                Type::Never
            }
            Expr::Return { value, .. } => {
                let found = value
                    .as_ref()
                    .map(|expression| {
                        if self.function_depth == 0 {
                            self.check_expr(expression)
                        } else {
                            self.check_expr_expected(expression, &self.return_type.clone())
                        }
                    })
                    .unwrap_or(Type::Unit);
                if self.function_depth == 0 {
                    self.errors.push(TypeError::OutsideFunction);
                } else {
                    if value.is_none() {
                        self.require_compatible(&self.return_type.clone(), &found);
                    }
                    if found != Type::Never {
                        if let Some(candidates) = self.return_candidates.last_mut() {
                            candidates.push(found);
                        }
                    }
                }
                Type::Never
            }
            Expr::Let {
                name,
                mutable,
                type_ann,
                value,
                ..
            } => {
                let (binding_type, value_type) = if let Some(annotation) = type_ann {
                    let binding_type = type_from_ast(annotation);
                    let value_type = self.check_expr_expected(value, &binding_type);
                    (binding_type, value_type)
                } else {
                    let value_type = self.check_expr(value);
                    (value_type.clone(), value_type)
                };
                self.define_mutable(name.clone(), binding_type.clone(), *mutable);
                if value_type == Type::Never {
                    Type::Never
                } else {
                    binding_type
                }
            }
            Expr::Assign {
                target, op, value, ..
            } => self.check_assignment(target, *op, value),
            Expr::Block(block) => self.check_block(block),
            Expr::Spawn { expr, .. } => {
                let raw = self.check_expr(expr);
                match self.resolve_alias(&raw) {
                    Type::Never => return Type::Never,
                    Type::Function(params, _) if params.is_empty() => {}
                    Type::Function(params, _) => self.errors.push(TypeError::Arity {
                        expected: 0,
                        found: params.len(),
                    }),
                    _ => self.errors.push(TypeError::NotCallable {
                        name: "spawn expression".into(),
                    }),
                }
                Type::Named("Task".into())
            }
            Expr::Try { expr, .. } => {
                let raw = self.check_expr(expr);
                match self.resolve_alias(&raw) {
                    Type::Never => Type::Never,
                    Type::Named(name) if name == "Option" || name == "Result" => {
                        // `?` can return the wrapper from the current function
                        // before the surrounding expression completes. Treat
                        // that path as a real return candidate and validate it
                        // against explicit contracts.
                        let wrapped = Type::Named(name);
                        if self.function_depth == 0 {
                            self.errors.push(TypeError::OutsideFunction);
                        } else {
                            self.require_compatible(&self.return_type.clone(), &wrapped);
                            if let Some(candidates) = self.return_candidates.last_mut() {
                                candidates.push(wrapped);
                            }
                        }
                        Type::Unknown
                    }
                    _ => {
                        self.errors.push(TypeError::InvalidTry);
                        Type::Unknown
                    }
                }
            }
            Expr::Closure {
                params,
                return_type,
                body,
                ..
            } => self.check_closure(params, return_type.as_ref(), body, None, None),
        }
    }

    /// Checks a closure with optional parameter/return context supplied by a
    /// function contract or a collection callback. Unannotated parameters must
    /// inherit that context before the body is checked; treating them as
    /// Unknown first allowed invalid bodies such as `|text| text * 2` to satisfy
    /// an unrelated `fn(string) -> int` contract.
    fn check_closure(
        &mut self,
        params: &[Param],
        declared_return: Option<&TypeExpr>,
        body: &Expr,
        contextual_params: Option<&[Type]>,
        contextual_result: Option<&Type>,
    ) -> Type {
        if let Some(expected) = contextual_params {
            if params.len() != expected.len() {
                self.errors.push(TypeError::Arity {
                    expected: expected.len(),
                    found: params.len(),
                });
            }
        }
        let mut parameter_names = HashSet::new();
        for parameter in params {
            if !parameter_names.insert(parameter.name.as_str()) {
                self.errors.push(TypeError::DuplicateDeclaration {
                    kind: "closure parameter".into(),
                    name: parameter.name.clone(),
                });
            }
        }

        self.push_scope();
        let parameter_types: Vec<Type> = params
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                parameter
                    .type_ann
                    .as_ref()
                    .map(type_from_ast)
                    .or_else(|| contextual_params.and_then(|types| types.get(index).cloned()))
                    .unwrap_or(Type::Unknown)
            })
            .collect();
        for (parameter, ty) in params.iter().zip(&parameter_types) {
            self.define_at_depth(
                parameter.name.clone(),
                ty.clone(),
                parameter.mutable,
                self.function_depth + 1,
            );
        }

        let declared_result = declared_return.map(type_from_ast);
        let expected_result = declared_result
            .clone()
            .or_else(|| contextual_result.cloned())
            .unwrap_or(Type::Unknown);
        let old_return = std::mem::replace(&mut self.return_type, expected_result.clone());
        let old_loop_depth = std::mem::replace(&mut self.loop_depth, 0);
        let old_loop_breaks = std::mem::take(&mut self.loop_breaks);
        self.function_depth += 1;
        self.return_candidates.push(Vec::new());
        let actual = self.check_expr(body);
        if actual != Type::Never {
            if let Some(candidates) = self.return_candidates.last_mut() {
                candidates.push(actual.clone());
            }
        }
        let candidates = self.return_candidates.pop().unwrap_or_default();
        self.function_depth -= 1;
        self.loop_depth = old_loop_depth;
        self.loop_breaks = old_loop_breaks;
        self.return_type = old_return;

        let result = declared_result
            .or_else(|| contextual_result.cloned())
            .unwrap_or_else(|| {
                if actual == Type::Never && candidates.is_empty() {
                    Type::Never
                } else {
                    self.infer_return_type("closure", &candidates)
                }
            });
        self.require_compatible(&result, &actual);
        self.pop_scope();
        Type::Function(parameter_types, Box::new(result))
    }

    /// Applies an expected type before checking closures, so their omitted
    /// parameter annotations are not silently treated as dynamic values.
    fn check_expr_expected(&mut self, expression: &Expr, expected: &Type) -> Type {
        let expected_resolved = self.resolve_alias(expected);
        let found = match (expression, &expected_resolved) {
            (
                Expr::Closure {
                    params,
                    return_type,
                    body,
                    ..
                },
                Type::Function(parameters, result),
            ) => self.check_closure(
                params,
                return_type.as_ref(),
                body,
                Some(parameters),
                Some(result),
            ),
            (Expr::Array { elements, .. }, Type::Array(item)) => {
                let mut reachable = true;
                for element in elements {
                    self.check_evaluated_expr(element, Some(item), &mut reachable);
                }
                if !reachable {
                    Type::Never
                } else {
                    Type::Array(item.clone())
                }
            }
            (Expr::Tuple { elements, .. }, Type::Tuple(items)) if elements.len() == items.len() => {
                let mut reachable = true;
                let found: Vec<_> = elements
                    .iter()
                    .zip(items)
                    .map(|(element, item)| {
                        self.check_evaluated_expr(element, Some(item), &mut reachable)
                    })
                    .collect();
                if !reachable {
                    Type::Never
                } else {
                    Type::Tuple(found)
                }
            }
            _ => self.check_expr(expression),
        };
        self.require_compatible(expected, &found);
        found
    }

    fn check_string_template(&mut self, template: &str) -> Type {
        let span = Default::default();
        let mut rest = template;
        let mut diverges = false;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('}') else {
                self.errors.push(TypeError::InvalidInterpolation {
                    expression: template.into(),
                });
                break;
            };
            let source = after[..close].trim();
            if let Some(open_call) = source.find('(') {
                if !source.ends_with(')') {
                    self.errors.push(TypeError::InvalidInterpolation {
                        expression: source.into(),
                    });
                } else {
                    let name = source[..open_call].trim();
                    let arguments = &source[open_call + 1..source.len() - 1];
                    if !is_template_path(name) {
                        self.errors.push(TypeError::InvalidInterpolation {
                            expression: source.into(),
                        });
                    } else {
                        let mut args = Vec::new();
                        let mut valid = true;
                        for argument in arguments
                            .split(',')
                            .map(str::trim)
                            .filter(|argument| !argument.is_empty())
                        {
                            if let Ok(value) = argument.parse::<i64>() {
                                args.push(Expr::Int { value, span });
                            } else if is_template_path(argument) && self.is_local_binding(argument)
                            {
                                args.push(Expr::Ident {
                                    name: argument.into(),
                                    span,
                                });
                            } else {
                                valid = false;
                                self.errors.push(TypeError::InvalidInterpolation {
                                    expression: argument.into(),
                                });
                            }
                        }
                        if valid {
                            let is_user_function = self.functions.contains_key(name)
                                && !self.base_functions.contains_key(name);
                            if is_user_function || titan_stdlib::native::contains(name) {
                                let result = self.check_call(
                                    &Expr::Ident {
                                        name: name.into(),
                                        span,
                                    },
                                    &args,
                                );
                                diverges |= self.resolve_alias(&result) == Type::Never;
                            } else {
                                self.errors.push(TypeError::InvalidInterpolation {
                                    expression: source.into(),
                                });
                            }
                        }
                    }
                }
            } else if is_template_path(source) {
                if self.is_local_binding(source) {
                    let result = self.check_expr(&Expr::Ident {
                        name: source.into(),
                        span,
                    });
                    diverges |= self.resolve_alias(&result) == Type::Never;
                } else {
                    self.errors.push(TypeError::InvalidInterpolation {
                        expression: source.into(),
                    });
                }
            } else {
                self.errors.push(TypeError::InvalidInterpolation {
                    expression: source.into(),
                });
            }
            rest = &after[close + 1..];
        }
        if diverges {
            Type::Never
        } else {
            Type::String
        }
    }

    fn check_method_call(&mut self, receiver: &Expr, method: &str, args: &[Expr]) -> Type {
        let mut reachable = true;
        let receiver_raw = self.check_evaluated_expr(receiver, None, &mut reachable);
        let receiver_type = self.resolve_alias(&receiver_raw);
        if receiver_type == Type::Never {
            for argument in args {
                self.check_evaluated_expr(argument, None, &mut reachable);
            }
            return Type::Never;
        }

        // These exact method shapes are lowered to dedicated bytecode by
        // codegen, before user-defined method dispatch. Validate the same
        // contracts here so code accepted by the checker cannot fail merely
        // because the intrinsic receives the wrong kind of value.
        match (method, args.len()) {
            ("len", 0) => {
                if is_length_supported(&receiver_type) {
                    return Type::Int;
                }
                self.errors.push(TypeError::UnknownMethod {
                    receiver: receiver_type,
                    method: method.into(),
                });
                return Type::Unknown;
            }
            ("map", 1) => {
                if let Some(item) = sequence_item_type(&receiver_type) {
                    let result = self.check_evaluated_callback(
                        "map callback",
                        &args[0],
                        &[item],
                        None,
                        &mut reachable,
                    );
                    return if reachable {
                        Type::Array(Box::new(result))
                    } else {
                        Type::Never
                    };
                }
                self.check_evaluated_expr(&args[0], None, &mut reachable);
                self.errors.push(TypeError::UnknownMethod {
                    receiver: receiver_type,
                    method: method.into(),
                });
                return if reachable {
                    Type::Unknown
                } else {
                    Type::Never
                };
            }
            ("filter", 1) => {
                if let Some(item) = sequence_item_type(&receiver_type) {
                    self.check_evaluated_callback(
                        "filter predicate",
                        &args[0],
                        &[item.clone()],
                        Some(&Type::Bool),
                        &mut reachable,
                    );
                    return if reachable {
                        Type::Array(Box::new(item))
                    } else {
                        Type::Never
                    };
                }
                self.check_evaluated_expr(&args[0], None, &mut reachable);
                self.errors.push(TypeError::UnknownMethod {
                    receiver: receiver_type,
                    method: method.into(),
                });
                return if reachable {
                    Type::Unknown
                } else {
                    Type::Never
                };
            }
            ("fold", 2) => {
                let initial = self.check_evaluated_expr(&args[0], None, &mut reachable);
                if let Some(item) = sequence_item_type(&receiver_type) {
                    self.check_evaluated_callback(
                        "fold callback",
                        &args[1],
                        &[initial.clone(), item],
                        Some(&initial),
                        &mut reachable,
                    );
                    return if reachable { initial } else { Type::Never };
                }
                self.check_evaluated_expr(&args[1], None, &mut reachable);
                self.errors.push(TypeError::UnknownMethod {
                    receiver: receiver_type,
                    method: method.into(),
                });
                return if reachable {
                    Type::Unknown
                } else {
                    Type::Never
                };
            }
            ("sort_by", 1) => {
                if let Some(item) = sequence_item_type(&receiver_type) {
                    let result = self.check_evaluated_callback(
                        "sort_by comparator",
                        &args[0],
                        &[item.clone(), item.clone()],
                        None,
                        &mut reachable,
                    );
                    if reachable
                        && result != Type::Never
                        && !is_numeric(&self.resolve_alias(&result))
                    {
                        self.errors.push(TypeError::Mismatch {
                            expected: Type::Int,
                            found: result,
                        });
                    }
                    return if reachable {
                        Type::Array(Box::new(item))
                    } else {
                        Type::Never
                    };
                }
                self.check_evaluated_expr(&args[0], None, &mut reachable);
                self.errors.push(TypeError::UnknownMethod {
                    receiver: receiver_type,
                    method: method.into(),
                });
                return if reachable {
                    Type::Unknown
                } else {
                    Type::Never
                };
            }
            ("find", 1) | ("any", 1) | ("all", 1) => {
                if let Some(item) = sequence_item_type(&receiver_type) {
                    self.check_evaluated_callback(
                        &format!("{method} predicate"),
                        &args[0],
                        &[item],
                        Some(&Type::Bool),
                        &mut reachable,
                    );
                    return if !reachable {
                        Type::Never
                    } else if method == "find" {
                        Type::Unknown
                    } else {
                        Type::Bool
                    };
                }
                self.check_evaluated_expr(&args[0], None, &mut reachable);
                self.errors.push(TypeError::UnknownMethod {
                    receiver: receiver_type,
                    method: method.into(),
                });
                return if reachable {
                    Type::Unknown
                } else {
                    Type::Never
                };
            }
            _ => {}
        }

        if let Type::Named(name) = &receiver_type {
            let qualified = format!("{}::{}", name, method);
            if let Some(signature) = self.functions.get(&qualified).cloned() {
                let Some((self_type, params)) = signature.params.split_first() else {
                    for arg in args {
                        self.check_evaluated_expr(arg, None, &mut reachable);
                    }
                    self.errors.push(TypeError::Arity {
                        expected: 0,
                        found: args.len() + 1,
                    });
                    return if reachable {
                        signature.result
                    } else {
                        Type::Never
                    };
                };
                self.require_compatible(self_type, &receiver_type);
                if params.len() != args.len() {
                    self.errors.push(TypeError::Arity {
                        expected: params.len(),
                        found: args.len(),
                    });
                }
                for (argument, expected) in args.iter().zip(params) {
                    self.check_evaluated_expr(argument, Some(expected), &mut reachable);
                }
                for argument in args.iter().skip(params.len()) {
                    self.check_evaluated_expr(argument, None, &mut reachable);
                }
                return if reachable {
                    signature.result
                } else {
                    Type::Never
                };
            }
        }

        for arg in args {
            self.check_evaluated_expr(arg, None, &mut reachable);
        }
        if receiver_type != Type::Unknown {
            if let Some(expected) = builtin_method_arity(&receiver_type, method) {
                self.errors.push(TypeError::Arity {
                    expected,
                    found: args.len(),
                });
            } else {
                self.errors.push(TypeError::UnknownMethod {
                    receiver: receiver_type,
                    method: method.into(),
                });
            }
        }
        if reachable {
            Type::Unknown
        } else {
            Type::Never
        }
    }

    fn check_evaluated_callback(
        &mut self,
        name: &str,
        expression: &Expr,
        arguments: &[Type],
        result: Option<&Type>,
        reachable: &mut bool,
    ) -> Type {
        let was_reachable = *reachable;
        let candidate_count = self.return_candidates.last().map(Vec::len);
        let loop_broke = self.loop_breaks.last().copied();
        let (found, diverges) =
            self.check_callback_expr_evaluation(name, expression, arguments, result);
        if !was_reachable {
            if let (Some(candidates), Some(count)) =
                (self.return_candidates.last_mut(), candidate_count)
            {
                candidates.truncate(count);
            }
            if let (Some(current), Some(previous)) = (self.loop_breaks.last_mut(), loop_broke) {
                *current = previous;
            }
        } else if diverges {
            *reachable = false;
        }
        found
    }

    fn check_callback_expr_evaluation(
        &mut self,
        name: &str,
        expression: &Expr,
        arguments: &[Type],
        result: Option<&Type>,
    ) -> (Type, bool) {
        let callable = if let Expr::Closure {
            params,
            return_type,
            body,
            ..
        } = expression
        {
            self.check_closure(params, return_type.as_ref(), body, Some(arguments), result)
        } else {
            self.check_expr(expression)
        };
        let diverges = self.resolve_alias(&callable) == Type::Never;
        (
            self.check_callback(name, &callable, arguments, result),
            diverges,
        )
    }

    fn check_callback(
        &mut self,
        name: &str,
        callable: &Type,
        arguments: &[Type],
        result: Option<&Type>,
    ) -> Type {
        match self.resolve_alias(callable) {
            Type::Function(params, found_result) => {
                if params.len() != arguments.len() {
                    self.errors.push(TypeError::Arity {
                        expected: arguments.len(),
                        found: params.len(),
                    });
                }
                for (parameter, argument) in params.iter().zip(arguments) {
                    if !function_parameter_accepts(parameter, argument) {
                        self.errors.push(TypeError::Mismatch {
                            expected: argument.clone(),
                            found: parameter.clone(),
                        });
                    }
                }
                if let Some(expected) = result {
                    if !function_result_compatible(expected, &found_result) {
                        self.errors.push(TypeError::Mismatch {
                            expected: expected.clone(),
                            found: *found_result.clone(),
                        });
                    }
                }
                *found_result
            }
            Type::Never => Type::Never,
            Type::Unknown => Type::Unknown,
            _ => {
                self.errors
                    .push(TypeError::NotCallable { name: name.into() });
                Type::Unknown
            }
        }
    }

    fn check_assignment(&mut self, target: &Expr, op: Option<BinaryOp>, value: &Expr) -> Type {
        let Expr::Ident { name, .. } = target else {
            let target_type = self.check_expr(target);
            let value_type = self.check_expr(value);
            self.errors.push(TypeError::InvalidAssignmentTarget);
            return if target_type == Type::Never || value_type == Type::Never {
                Type::Never
            } else {
                Type::Unknown
            };
        };
        let Some(target_type) = self.lookup(name) else {
            self.errors
                .push(TypeError::UnknownVariable { name: name.clone() });
            let value_type = self.check_expr(value);
            self.errors.push(TypeError::InvalidAssignmentTarget);
            return if value_type == Type::Never {
                Type::Never
            } else {
                Type::Unknown
            };
        };
        if self.is_global_constant_binding(name) {
            let value_type = self.check_expr(value);
            self.errors.push(TypeError::InvalidAssignmentTarget);
            return if value_type == Type::Never {
                Type::Never
            } else {
                target_type
            };
        }
        let mutable_in_this_function = self
            .binding(name)
            .is_some_and(|(mutable, depth)| mutable && depth == self.function_depth);
        if !mutable_in_this_function {
            let value_type = self.check_expr(value);
            self.errors.push(TypeError::InvalidAssignmentTarget);
            return if value_type == Type::Never {
                Type::Never
            } else {
                target_type
            };
        }
        let value_type = if let Some(operator) = op {
            let value_type = self.check_expr(value);
            let result = if value_type == Type::Never {
                Type::Never
            } else {
                self.check_binary(operator, target_type.clone(), value_type.clone())
            };
            self.require_compatible(&target_type, &result);
            value_type
        } else {
            self.check_expr_expected(value, &target_type)
        };
        if value_type == Type::Never {
            Type::Never
        } else {
            target_type
        }
    }

    fn is_local_binding(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .skip(1)
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn is_global_constant_binding(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .enumerate()
            .rev()
            .find(|(_, scope)| scope.contains_key(name))
            .is_some_and(|(index, _)| index == 0 && self.constants.contains(name))
    }

    fn check_collection_call(&mut self, name: &str, args: &[Expr]) -> Option<Type> {
        let expected_arity = match name {
            "len" => 1,
            "map" | "filter" | "sort_by" | "find" | "any" | "all" => 2,
            "fold" => 3,
            _ => return None,
        };
        // The ordinary call checker reports malformed arity and still visits
        // every argument. Only apply the intrinsic contract when codegen will
        // lower the exact call shape to a collection opcode.
        if args.len() != expected_arity {
            return None;
        }
        if name == "len" {
            let raw = self.check_expr(&args[0]);
            let found = self.resolve_alias(&raw);
            if found == Type::Never {
                return Some(Type::Never);
            }
            if !is_length_supported(&found) {
                self.errors.push(TypeError::InvalidArgument {
                    function: name.into(),
                    expected: "array, tuple, string, bytes, or map".into(),
                    found,
                });
            }
            return Some(Type::Int);
        }

        let mut reachable = true;
        let raw = self.check_evaluated_expr(&args[0], None, &mut reachable);
        let sequence = self.resolve_alias(&raw);
        if sequence == Type::Never {
            for argument in &args[1..] {
                self.check_evaluated_expr(argument, None, &mut reachable);
            }
            return Some(Type::Never);
        }
        let item = match sequence_item_type(&sequence) {
            Some(item) => item,
            None => {
                self.errors.push(TypeError::InvalidArgument {
                    function: name.into(),
                    expected: "array or tuple".into(),
                    found: sequence,
                });
                Type::Unknown
            }
        };
        let result = match name {
            "map" => {
                let output = self.check_evaluated_callback(
                    "map callback",
                    &args[1],
                    &[item],
                    None,
                    &mut reachable,
                );
                Type::Array(Box::new(output))
            }
            "filter" => {
                self.check_evaluated_callback(
                    "filter predicate",
                    &args[1],
                    &[item.clone()],
                    Some(&Type::Bool),
                    &mut reachable,
                );
                Type::Array(Box::new(item))
            }
            "fold" => {
                let initial = self.check_evaluated_expr(&args[1], None, &mut reachable);
                self.check_evaluated_callback(
                    "fold callback",
                    &args[2],
                    &[initial.clone(), item],
                    Some(&initial),
                    &mut reachable,
                );
                initial
            }
            "sort_by" => {
                let output = self.check_evaluated_callback(
                    "sort_by comparator",
                    &args[1],
                    &[item.clone(), item.clone()],
                    None,
                    &mut reachable,
                );
                if reachable && output != Type::Never && !is_numeric(&self.resolve_alias(&output)) {
                    self.errors.push(TypeError::Mismatch {
                        expected: Type::Int,
                        found: output,
                    });
                }
                Type::Array(Box::new(item))
            }
            "find" => {
                self.check_evaluated_callback(
                    "find predicate",
                    &args[1],
                    &[item],
                    Some(&Type::Bool),
                    &mut reachable,
                );
                Type::Unknown
            }
            "any" | "all" => {
                self.check_evaluated_callback(
                    &format!("{name} predicate"),
                    &args[1],
                    &[item],
                    Some(&Type::Bool),
                    &mut reachable,
                );
                Type::Bool
            }
            _ => unreachable!("collection intrinsic was filtered above"),
        };
        if reachable {
            Some(result)
        } else {
            Some(Type::Never)
        }
    }

    fn check_try_catch_call(&mut self, args: &[Expr]) -> Type {
        let result_type = Type::Named("Result".into());
        let Some((callable_expression, call_args)) = args.split_first() else {
            self.errors.push(TypeError::Arity {
                expected: 1,
                found: 0,
            });
            return result_type;
        };

        let mut reachable = true;
        // Inline closures need the argument types as context, but constructing
        // the closure itself cannot diverge. Other callable expressions are
        // evaluated before the invocation arguments, matching codegen.
        let inline_closure = matches!(callable_expression, Expr::Closure { .. });
        let callable = if inline_closure {
            None
        } else {
            Some(self.check_evaluated_expr(callable_expression, None, &mut reachable))
        };
        let argument_types: Vec<Type> = call_args
            .iter()
            .map(|argument| self.check_evaluated_expr(argument, None, &mut reachable))
            .collect();
        let callable = if let Expr::Closure {
            params,
            return_type,
            body,
            ..
        } = callable_expression
        {
            self.check_closure(
                params,
                return_type.as_ref(),
                body,
                Some(&argument_types),
                None,
            )
        } else {
            callable.unwrap_or(Type::Unknown)
        };

        match self.resolve_alias(&callable) {
            Type::Never => return Type::Never,
            Type::Function(params, _) => {
                if params.len() != argument_types.len() {
                    self.errors.push(TypeError::Arity {
                        expected: params.len(),
                        found: argument_types.len(),
                    });
                }
                for (expected, found) in params.iter().zip(&argument_types) {
                    self.require_compatible(expected, found);
                }
            }
            _ => self.errors.push(TypeError::NotCallable {
                name: "first argument to std::try::catch".into(),
            }),
        }
        if reachable {
            result_type
        } else {
            Type::Never
        }
    }

    /// Typechecks an expression that is evaluated after earlier siblings.
    /// Diagnostics are still produced for unreachable syntax, but `return`
    /// candidates from that syntax must not affect the enclosing function's
    /// inferred result.
    fn check_evaluated_expr(
        &mut self,
        expression: &Expr,
        expected: Option<&Type>,
        reachable: &mut bool,
    ) -> Type {
        let was_reachable = *reachable;
        let candidate_count = self.return_candidates.last().map(Vec::len);
        let loop_broke = self.loop_breaks.last().copied();
        let found = if let Some(expected) = expected {
            self.check_expr_expected(expression, expected)
        } else {
            self.check_expr(expression)
        };
        if !was_reachable {
            if let (Some(candidates), Some(count)) =
                (self.return_candidates.last_mut(), candidate_count)
            {
                candidates.truncate(count);
            }
            if let (Some(current), Some(previous)) = (self.loop_breaks.last_mut(), loop_broke) {
                *current = previous;
            }
        } else if self.resolve_alias(&found) == Type::Never {
            *reachable = false;
        }
        found
    }

    /// Dedicated VM operations sometimes accept a runtime shape that cannot be
    /// expressed by Titan's current source-level types (for example, TCP close
    /// accepts either a stream or a listener, and database operations accept
    /// any of three connection handles). These built-ins are direct-call only,
    /// so enforce their precise runtime contracts at that boundary without
    /// weakening `any` or ordinary user function parameters.
    fn check_dedicated_call_argument(
        &mut self,
        function: &str,
        index: usize,
        expression: &Expr,
        expected: &Type,
        reachable: &mut bool,
    ) -> bool {
        let accepts_utf8 = index == 1
            && matches!(
                function,
                "std::net::tcp_write" | "std::tls::write"
            );
        let sequence_item = match (function, index) {
            ("select", 0) => Some(Type::Named("Receiver".into())),
            (
                "std::sqlite::execute"
                | "std::sqlite::query"
                | "std::postgres::execute"
                | "std::postgres::query"
                | "std::mysql::execute"
                | "std::mysql::query"
                | "std::db::execute"
                | "std::db::query",
                2,
            ) => Some(Type::Unknown),
            (
                "std::sqlite::migrate"
                | "std::postgres::migrate"
                | "std::mysql::migrate"
                | "std::db::migrate",
                1,
            ) => Some(Type::Named("map".into())),
            _ => None,
        };
        let tcp_handle = function == "std::net::tcp_close" && index == 0;
        let database_handle = index == 0
            && matches!(
                function,
                "std::db::execute"
                    | "std::db::query"
                    | "std::db::begin"
                    | "std::db::commit"
                    | "std::db::rollback"
                    | "std::db::migrate"
                    | "std::db::close"
                    | "std::db::ping"
            );
        if !accepts_utf8 && sequence_item.is_none() && !tcp_handle && !database_handle {
            return false;
        }

        let found = self.check_evaluated_expr(expression, None, reachable);
        let found_resolved = self.resolve_alias(&found);
        if accepts_utf8 {
            let expected_resolved = self.resolve_alias(expected);
            if !compatible(&expected_resolved, &found_resolved)
                && !(matches!(&expected_resolved, Type::Named(name) if name == "bytes")
                    && found_resolved == Type::String)
            {
                self.errors.push(TypeError::Mismatch {
                    expected: expected.clone(),
                    found,
                });
            }
        } else if let Some(item) = sequence_item {
            if !sequence_matches(&item, &found_resolved) {
                self.errors.push(TypeError::Mismatch {
                    expected: Type::Array(Box::new(item)),
                    found,
                });
            }
        } else {
            let valid = matches!(&found_resolved, Type::Unknown | Type::Never)
                || matches!(
                    (&found_resolved, tcp_handle, database_handle),
                    (Type::Named(name), true, false)
                        if matches!(name.as_str(), "TcpStream" | "TcpListener")
                )
                || matches!(
                    (&found_resolved, tcp_handle, database_handle),
                    (Type::Named(name), false, true)
                        if matches!(name.as_str(), "Sqlite" | "Postgres" | "Mysql")
                );
            if !valid {
                self.errors.push(TypeError::InvalidArgument {
                    function: function.into(),
                    expected: if tcp_handle {
                        "TCP stream or listener".into()
                    } else {
                        "SQLite, PostgreSQL, or MySQL connection".into()
                    },
                    found,
                });
            }
        }
        true
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr]) -> Type {
        let name = if let Expr::Ident { name, .. } = callee {
            Some(name.clone())
        } else {
            None
        };
        // Codegen gives local values priority over statically named builtins.
        // Apply intrinsic/native/constructor contracts only when this name is
        // not shadowed by a parameter or local binding; otherwise the ordinary
        // callable path below must validate the actual local function type.
        let static_name = name
            .as_ref()
            .filter(|name| !self.is_local_binding(name.as_str()));
        let variadic_print =
            static_name.is_some_and(|name| matches!(name.as_str(), "print" | "println"));
        if let Some(name) = static_name {
            if let Some(result) = self.check_collection_call(name, args) {
                return result;
            }
            if name == "std::try::catch" {
                return self.check_try_catch_call(args);
            }
            if let Some(signature) = titan_stdlib::native::lookup(name) {
                if args.len() != signature.params.len() {
                    self.errors.push(TypeError::Arity {
                        expected: signature.params.len(),
                        found: args.len(),
                    });
                }
                let mut reachable = true;
                for (argument, expected) in args.iter().zip(signature.params) {
                    let found = self.check_evaluated_expr(argument, None, &mut reachable);
                    let found = self.resolve_alias(&found);
                    let expected = native_type(*expected);
                    if !native_compatible(&expected, &found) {
                        self.errors.push(TypeError::Mismatch { expected, found });
                    }
                }
                for argument in args.iter().skip(signature.params.len()) {
                    self.check_evaluated_expr(argument, None, &mut reachable);
                }
                return if reachable {
                    native_type(signature.result)
                } else {
                    Type::Never
                };
            }
            if let Some(payload) = self.enum_variants.get(name).cloned() {
                let expected = usize::from(payload.is_some());
                if args.len() != expected {
                    self.errors.push(TypeError::Arity {
                        expected,
                        found: args.len(),
                    });
                }
                let mut reachable = true;
                let checked = if let (Some(expected), Some(argument)) = (payload, args.first()) {
                    self.check_evaluated_expr(argument, Some(&expected), &mut reachable);
                    1
                } else {
                    0
                };
                for argument in args.iter().skip(checked) {
                    self.check_evaluated_expr(argument, None, &mut reachable);
                }
                return if reachable {
                    Type::Named(
                        name.split_once("::")
                            .map_or(name.as_str(), |(e, _)| e)
                            .into(),
                    )
                } else {
                    Type::Never
                };
            }
        }
        let mut reachable = true;
        // VM-dedicated built-ins are callable by their static name, but they
        // are not first-class closure values in codegen. Use the registered
        // signature directly for this call position; check_identifier rejects
        // the same name when it appears as an ordinary value expression.
        let builtin_signature = static_name
            .and_then(|name| self.base_functions.get(name.as_str()))
            .cloned();
        let ty = if let Some(signature) = builtin_signature {
            Type::Function(signature.params, Box::new(signature.result))
        } else {
            self.check_evaluated_expr(callee, None, &mut reachable)
        };
        // Phase 32 fix: expand type aliases so `Named("Callback")`
        // (where `type Callback = fn(int) -> int`) resolves to the
        // real Function(...) shape before the callable check.
        let ty = self.resolve_alias(&ty);
        match ty {
            Type::Never => {
                for arg in args {
                    self.check_evaluated_expr(arg, None, &mut reachable);
                }
                Type::Never
            }
            Type::Function(params, result) => {
                if params.len() != args.len() && !variadic_print {
                    self.errors.push(TypeError::Arity {
                        expected: params.len(),
                        found: args.len(),
                    });
                }
                for (index, (arg, expected)) in args.iter().zip(&params).enumerate() {
                    let dedicated = static_name.is_some_and(|function| {
                        self.check_dedicated_call_argument(
                            function,
                            index,
                            arg,
                            expected,
                            &mut reachable,
                        )
                    });
                    if !dedicated {
                        self.check_evaluated_expr(arg, Some(expected), &mut reachable);
                    }
                }
                for arg in args.iter().skip(params.len()) {
                    self.check_evaluated_expr(arg, None, &mut reachable);
                }
                if reachable {
                    *result
                } else {
                    Type::Never
                }
            }
            Type::Unknown => {
                for arg in args {
                    self.check_evaluated_expr(arg, None, &mut reachable);
                }
                if reachable {
                    Type::Unknown
                } else {
                    Type::Never
                }
            }
            _ => {
                for arg in args {
                    self.check_evaluated_expr(arg, None, &mut reachable);
                }
                self.errors.push(TypeError::NotCallable {
                    name: name.unwrap_or_else(|| "expression".into()),
                });
                if reachable {
                    Type::Unknown
                } else {
                    Type::Never
                }
            }
        }
    }

    fn check_binary(&mut self, op: BinaryOp, left: Type, right: Type) -> Type {
        use BinaryOp::*;
        match op {
            Eq | Neq => {
                self.require_compatible(&left, &right);
                Type::Bool
            }
            Lt | Gt | Lte | Gte => {
                if !is_numeric(&left) || !compatible(&left, &right) {
                    self.invalid(op, left, right);
                }
                Type::Bool
            }
            LazyAnd | LazyOr => {
                self.require_compatible(&Type::Bool, &left);
                self.require_compatible(&Type::Bool, &right);
                Type::Bool
            }
            // v0.16.0 QoL: String + Any coerces to String at runtime via
            // val_to_string(), so any of these are safe:
            //   "x " + int, "x " + float, "x " + array, "x " + unknown
            // The old rule (both must be String) forced ugly workarounds
            // like putting every non-String inside string interpolation
            // ({var}) which is verbose. Now `+` mirrors Python's f-strings
            // and JavaScript's `${}`: whenever the left is a String, the
            // whole expression is a String.
            Add if left == Type::String => Type::String,
            Add if right == Type::String => Type::String,
            Add | Sub | Mul | Div if is_numeric(&left) && compatible(&left, &right) => left,
            Mod if compatible(&Type::Int, &left) && compatible(&Type::Int, &right) => Type::Int,
            And | Or | Xor
                if compatible(&Type::Int, &left) && compatible(&Type::Int, &right) =>
            {
                Type::Int
            }
            _ => {
                self.invalid(op, left, right);
                Type::Unknown
            }
        }
    }

    fn invalid(&mut self, op: BinaryOp, left: Type, right: Type) {
        self.errors.push(TypeError::InvalidOperands {
            operator: format!("{op:?}"),
            left,
            right,
        });
    }
    /// Expands aliases recursively without an arbitrary chain limit. Alias
    /// cycles are diagnosed during declaration validation; the visited set is
    /// still kept here as a defensive boundary for embedders constructing an
    /// AST directly.
    fn resolve_alias(&self, ty: &Type) -> Type {
        self.resolve_alias_inner(ty, &mut HashSet::new())
    }

    fn resolve_alias_inner(&self, ty: &Type, visited: &mut HashSet<String>) -> Type {
        match ty {
            Type::Named(name) => {
                let Some(target) = self.type_aliases.get(name) else {
                    return ty.clone();
                };
                if !visited.insert(name.clone()) {
                    return Type::Unknown;
                }
                let resolved = self.resolve_alias_inner(target, visited);
                visited.remove(name);
                resolved
            }
            Type::Array(inner) => Type::Array(Box::new(self.resolve_alias_inner(inner, visited))),
            Type::Tuple(items) => Type::Tuple(
                items
                    .iter()
                    .map(|item| self.resolve_alias_inner(item, visited))
                    .collect(),
            ),
            Type::Function(params, result) => Type::Function(
                params
                    .iter()
                    .map(|param| self.resolve_alias_inner(param, visited))
                    .collect(),
                Box::new(self.resolve_alias_inner(result, visited)),
            ),
            _ => ty.clone(),
        }
    }
    fn require_compatible(&mut self, expected: &Type, found: &Type) {
        let expected_r = self.resolve_alias(expected);
        let found_r = self.resolve_alias(found);
        if !compatible(&expected_r, &found_r) {
            self.errors.push(TypeError::Mismatch {
                expected: expected.clone(),
                found: found.clone(),
            });
        }
    }
    fn enum_variant_names(&self, enumeration: &str) -> HashSet<String> {
        let prefix = format!("{}::", enumeration);
        self.enum_variants
            .keys()
            .filter_map(|qualified| qualified.strip_prefix(&prefix).map(str::to_string))
            .collect()
    }

    fn match_domain(&self, subject: &Type) -> MatchDomain {
        match self.resolve_alias(subject) {
            Type::Never => MatchDomain::Empty,
            Type::Bool => MatchDomain::Bool,
            Type::Named(name) => {
                let variants = self.enum_variant_names(&name);
                if variants.is_empty() {
                    MatchDomain::Open
                } else {
                    MatchDomain::Enum { name, variants }
                }
            }
            // A dynamic value may contain more than the variants mentioned by
            // the source. Enum-looking arms therefore cannot make an `any`
            // match exhaustive without a real catch-all.
            Type::Unknown => MatchDomain::Open,
            _ => MatchDomain::Open,
        }
    }

    /// Returns the exact runtime atom produced by a syntactically known match
    /// subject. Codegen evaluates arms in order, so later non-matching arms (or
    /// arms after an unguarded match) cannot contribute values, returns, or
    /// breaks. Keep this deliberately limited to expressions whose constructor
    /// is unambiguous without executing user code.
    fn known_match_atom(&self, expression: &Expr, domain: &MatchDomain) -> Option<MatchAtom> {
        match (domain, expression) {
            (MatchDomain::Bool, Expr::Bool { value, .. }) => Some(MatchAtom::Bool(*value)),
            (MatchDomain::Open, Expr::Int { value, .. }) => Some(MatchAtom::Int(*value)),
            (MatchDomain::Open, Expr::String { value, .. }) => {
                Some(MatchAtom::String(value.clone()))
            }
            (MatchDomain::Open, Expr::Char { value, .. }) => Some(MatchAtom::Char(*value)),
            (MatchDomain::Open, Expr::Nil { .. }) => Some(MatchAtom::Nil),
            (
                MatchDomain::Enum {
                    name: enumeration,
                    variants,
                },
                Expr::Ident { name, .. },
            ) => {
                let (found_enumeration, variant) = name.split_once("::")?;
                (found_enumeration == enumeration
                    && variants.contains(variant)
                    && self.enum_variants.get(name).is_some_and(Option::is_none))
                .then(|| MatchAtom::EnumVariant(variant.into()))
            }
            (
                MatchDomain::Enum {
                    name: enumeration,
                    variants,
                },
                Expr::Call { callee, args, .. },
            ) => {
                let Expr::Ident { name, .. } = callee.as_ref() else {
                    return None;
                };
                let (found_enumeration, variant) = name.split_once("::")?;
                let payload = self.enum_variants.get(name)?;
                (found_enumeration == enumeration
                    && variants.contains(variant)
                    && args.len() == usize::from(payload.is_some()))
                .then(|| MatchAtom::EnumVariant(variant.into()))
            }
            _ => None,
        }
    }

    fn pattern_coverage(&self, pattern: &Pattern, domain: &MatchDomain) -> Option<PatternCoverage> {
        match pattern {
            Pattern::Wildcard { .. } | Pattern::Ident { .. } => Some(PatternCoverage {
                all: true,
                atoms: HashSet::new(),
            }),
            Pattern::Literal { value, .. } => {
                let atom = match (domain, value.as_ref()) {
                    (MatchDomain::Bool, Expr::Bool { value, .. }) => MatchAtom::Bool(*value),
                    (MatchDomain::Open, Expr::Int { value, .. }) => MatchAtom::Int(*value),
                    (MatchDomain::Open, Expr::String { value, .. }) => {
                        MatchAtom::String(value.clone())
                    }
                    (MatchDomain::Open, Expr::Char { value, .. }) => MatchAtom::Char(*value),
                    (MatchDomain::Open, Expr::Nil { .. }) => MatchAtom::Nil,
                    _ => return None,
                };
                Some(PatternCoverage {
                    all: false,
                    atoms: HashSet::from([atom]),
                })
            }
            Pattern::Enum {
                name,
                variant,
                inner,
                ..
            } => {
                let MatchDomain::Enum {
                    name: enumeration,
                    variants,
                } = domain
                else {
                    return None;
                };
                if name != enumeration || !variants.contains(variant) {
                    return None;
                }
                let qualified = format!("{}::{}", name, variant);
                let covers_variant = match (self.enum_variants.get(&qualified), inner) {
                    (Some(None), None) => true,
                    (Some(Some(_)), Some(inner)) => pattern_is_catchall(inner),
                    _ => false,
                };
                covers_variant.then(|| PatternCoverage {
                    all: false,
                    atoms: HashSet::from([MatchAtom::EnumVariant(variant.clone())]),
                })
            }
            Pattern::Or { left, right, .. } => {
                let mut left = self.pattern_coverage(left, domain)?;
                let right = self.pattern_coverage(right, domain)?;
                left.all |= right.all;
                left.atoms.extend(right.atoms);
                Some(left)
            }
            Pattern::Tuple { .. } | Pattern::Struct { .. } => None,
        }
    }

    fn check_match_coverage(&mut self, subject: &Type, arms: &[MatchArm]) {
        let domain = self.match_domain(subject);
        let mut coverage = MatchCoverage::default();

        for (index, arm) in arms.iter().enumerate() {
            let pattern = self.pattern_coverage(&arm.pattern, &domain);
            let unreachable = matches!(&domain, MatchDomain::Empty)
                || coverage.all
                || pattern
                    .as_ref()
                    .is_some_and(|pattern| coverage.covers(pattern, &domain));
            if unreachable {
                self.errors
                    .push(TypeError::UnreachablePattern { arm: index + 1 });
            }
            // A guard can fail at runtime, so guarded arms never contribute to
            // exhaustiveness or make a later arm unreachable.
            if arm.guard.is_none() {
                if let Some(pattern) = pattern {
                    coverage.add(pattern);
                }
            }
        }

        match domain {
            MatchDomain::Empty => {}
            MatchDomain::Bool => {
                if !coverage.is_complete(&MatchDomain::Bool) {
                    self.errors.push(TypeError::NonExhaustiveMatch);
                }
            }
            MatchDomain::Enum { name, variants } => {
                if !coverage.all {
                    let mut missing: Vec<_> = variants
                        .into_iter()
                        .filter(|variant| {
                            !coverage
                                .atoms
                                .contains(&MatchAtom::EnumVariant(variant.clone()))
                        })
                        .collect();
                    missing.sort();
                    if !missing.is_empty() {
                        self.errors.push(TypeError::NonExhaustiveEnum {
                            enumeration: name,
                            missing,
                        });
                    }
                }
            }
            MatchDomain::Open => {
                if !coverage.all {
                    self.errors.push(TypeError::NonExhaustiveMatch);
                }
            }
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern, subject: &Type) {
        match pattern {
            Pattern::Wildcard { .. } => {}
            Pattern::Ident { name, .. } => {
                self.define(name.clone(), subject.clone());
            }
            Pattern::Literal { value, .. } => {
                let found = self.check_expr(value);
                self.require_compatible(subject, &found);
            }
            Pattern::Or { left, right, .. } => {
                let original = self.scopes.last().cloned().unwrap_or_default();
                let original_bindings = self.bindings.last().cloned().unwrap_or_default();
                self.bind_pattern(left, subject);
                let left_scope = self.scopes.last().cloned().unwrap_or_default();
                let left_names = pattern_binding_names(left);

                if let Some(scope) = self.scopes.last_mut() {
                    *scope = original.clone();
                }
                if let Some(bindings) = self.bindings.last_mut() {
                    *bindings = original_bindings.clone();
                }
                self.bind_pattern(right, subject);
                let right_scope = self.scopes.last().cloned().unwrap_or_default();
                let right_names = pattern_binding_names(right);

                if let Some(scope) = self.scopes.last_mut() {
                    *scope = original;
                }
                if let Some(bindings) = self.bindings.last_mut() {
                    *bindings = original_bindings;
                }
                if left_names != right_names {
                    self.errors.push(TypeError::InvalidPattern {
                        message: "both sides of an or-pattern must bind the same names".into(),
                    });
                }
                for name in left_names.intersection(&right_names) {
                    let left_type = left_scope.get(name).cloned().unwrap_or(Type::Unknown);
                    let right_type = right_scope.get(name).cloned().unwrap_or(Type::Unknown);
                    self.require_compatible(&left_type, &right_type);
                    self.define(name.clone(), left_type);
                }
            }
            Pattern::Enum {
                name,
                variant,
                inner,
                ..
            } => {
                self.require_compatible(&Type::Named(name.clone()), subject);
                let qualified = format!("{}::{}", name, variant);
                match self.enum_variants.get(&qualified).cloned() {
                    Some(payload) => match (payload, inner) {
                        (Some(payload), Some(inner)) => self.bind_pattern(inner, &payload),
                        (None, Some(inner)) => {
                            self.errors.push(TypeError::InvalidPattern {
                                message: format!("variant '{}::{}' has no payload", name, variant),
                            });
                            self.bind_pattern(inner, &Type::Unknown);
                        }
                        (Some(_), None) => {
                            self.errors.push(TypeError::InvalidPattern {
                                message: format!(
                                    "variant '{}::{}' requires a payload pattern",
                                    name, variant
                                ),
                            });
                        }
                        (None, None) => {}
                    },
                    None => {
                        self.errors.push(TypeError::UnknownVariant {
                            enumeration: name.clone(),
                            variant: variant.clone(),
                        });
                        if let Some(inner) = inner {
                            self.bind_pattern(inner, &Type::Unknown);
                        }
                    }
                }
            }
            Pattern::Tuple { elements, .. } => match self.resolve_alias(subject) {
                Type::Tuple(subjects) => {
                    if subjects.len() != elements.len() {
                        self.errors.push(TypeError::InvalidPattern {
                            message: format!(
                                "tuple pattern has {} elements but value has {}",
                                elements.len(),
                                subjects.len()
                            ),
                        });
                    }
                    for (index, element) in elements.iter().enumerate() {
                        let element_type = subjects.get(index).cloned().unwrap_or(Type::Unknown);
                        self.bind_pattern(element, &element_type);
                    }
                }
                Type::Unknown => {
                    for element in elements {
                        self.bind_pattern(element, &Type::Unknown);
                    }
                }
                found => {
                    self.errors.push(TypeError::InvalidPattern {
                        message: format!("tuple pattern cannot match {found}"),
                    });
                    for element in elements {
                        self.bind_pattern(element, &Type::Unknown);
                    }
                }
            },
            Pattern::Struct {
                name, fields, rest, ..
            } => {
                self.require_compatible(&Type::Named(name.clone()), subject);
                let Some(schema) = self.structs.get(name).cloned() else {
                    self.errors
                        .push(TypeError::UnknownType { name: name.clone() });
                    for (_, pattern) in fields {
                        self.bind_pattern(pattern, &Type::Unknown);
                    }
                    return;
                };
                let mut supplied = HashSet::new();
                for (field, pattern) in fields {
                    if !supplied.insert(field.as_str()) {
                        self.errors.push(TypeError::InvalidPattern {
                            message: format!(
                                "field '{}.{}' appears more than once in the pattern",
                                name, field
                            ),
                        });
                    }
                    let field_type = schema.get(field).cloned().unwrap_or_else(|| {
                        self.errors.push(TypeError::UnknownField {
                            structure: name.clone(),
                            field: field.clone(),
                        });
                        Type::Unknown
                    });
                    self.bind_pattern(pattern, &field_type);
                }
                if !rest {
                    for field in schema.keys() {
                        if !supplied.contains(field.as_str()) {
                            self.errors.push(TypeError::MissingField {
                                structure: name.clone(),
                                field: field.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.bindings.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        self.scopes.pop();
        self.bindings.pop();
    }
    fn define(&mut self, name: String, ty: Type) {
        self.define_at_depth(name, ty, false, self.function_depth);
    }
    fn define_mutable(&mut self, name: String, ty: Type, mutable: bool) {
        self.define_at_depth(name, ty, mutable, self.function_depth);
    }
    fn define_at_depth(&mut self, name: String, ty: Type, mutable: bool, depth: usize) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.clone(), ty);
        }
        if let Some(bindings) = self.bindings.last_mut() {
            bindings.insert(name, (mutable, depth));
        }
    }
    fn lookup(&self, name: &str) -> Option<Type> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }
    fn binding(&self, name: &str) -> Option<(bool, usize)> {
        self.bindings
            .iter()
            .rev()
            .find_map(|bindings| bindings.get(name).copied())
    }
}

fn is_template_path(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    value.replace("::", ".").split('.').all(|segment| {
        let mut chars = segment.chars();
        chars
            .next()
            .is_some_and(|first| first == '_' || first.is_alphabetic())
            && chars.all(|character| character == '_' || character.is_alphanumeric())
    })
}

fn count_inference_targets(items: &[Item]) -> usize {
    items
        .iter()
        .map(|item| match item {
            Item::Function(_) | Item::Const(_) => 1,
            Item::Impl(block) => block.methods.len(),
            Item::Trait(trait_decl) => trait_decl.methods.len(),
            Item::Module(module) => count_inference_targets(&module.items),
            _ => 0,
        })
        .sum()
}

fn declaration_errors(
    base_functions: &HashMap<String, FunctionSig>,
    items: &[Item],
) -> Vec<TypeError> {
    struct State {
        values: HashSet<String>,
        types: HashSet<String>,
        impls: HashSet<String>,
        errors: Vec<TypeError>,
    }

    fn insert(state: &mut State, namespace: &str, kind: &str, name: &str) {
        let names = if namespace == "value" {
            &mut state.values
        } else {
            &mut state.types
        };
        if !names.insert(name.into()) {
            state.errors.push(TypeError::DuplicateDeclaration {
                kind: kind.into(),
                name: name.into(),
            });
        }
    }

    fn parameters(state: &mut State, owner: &str, params: &[Param]) {
        let mut names = HashSet::new();
        for param in params {
            if !names.insert(param.name.as_str()) {
                state.errors.push(TypeError::DuplicateDeclaration {
                    kind: "parameter".into(),
                    name: format!("{}::{}", owner, param.name),
                });
            }
            if param.default.is_some() {
                state.errors.push(TypeError::UnsupportedFeature {
                    feature: format!("default parameter '{}::{}'", owner, param.name),
                });
            }
        }
    }

    fn reject_unimplemented_function(state: &mut State, owner: &str, function: &FunctionDecl) {
        if function.is_extern {
            state.errors.push(TypeError::UnsupportedFeature {
                feature: format!("extern function '{}' without runtime linkage", owner),
            });
        } else if function.body.is_none() {
            state.errors.push(TypeError::UnsupportedFeature {
                feature: format!("bodyless function '{}' outside a trait declaration", owner),
            });
        }
    }

    fn visit(state: &mut State, items: &[Item]) {
        for item in items {
            match item {
                Item::Function(function) => {
                    insert(state, "value", "function", &function.name);
                    parameters(state, &function.name, &function.params);
                    reject_unimplemented_function(state, &function.name, function);
                }
                Item::Const(constant) => insert(state, "value", "constant", &constant.name),
                Item::Struct(structure) => {
                    insert(state, "type", "type", &structure.name);
                    let mut fields = HashSet::new();
                    for field in &structure.fields {
                        if !fields.insert(field.name.as_str()) {
                            state.errors.push(TypeError::DuplicateDeclaration {
                                kind: "field".into(),
                                name: format!("{}::{}", structure.name, field.name),
                            });
                        }
                    }
                }
                Item::Enum(enumeration) => {
                    insert(state, "type", "type", &enumeration.name);
                    let mut variants = HashSet::new();
                    for variant in &enumeration.variants {
                        if !variants.insert(variant.name.as_str()) {
                            state.errors.push(TypeError::DuplicateDeclaration {
                                kind: "enum variant".into(),
                                name: format!("{}::{}", enumeration.name, variant.name),
                            });
                        }
                    }
                }
                Item::Trait(trait_decl) => {
                    insert(state, "type", "trait", &trait_decl.name);
                    let mut methods = HashSet::new();
                    for method in &trait_decl.methods {
                        if !methods.insert(method.name.as_str()) {
                            state.errors.push(TypeError::DuplicateDeclaration {
                                kind: "trait method".into(),
                                name: format!("{}::{}", trait_decl.name, method.name),
                            });
                        }
                        let qualified = format!("{}::{}", trait_decl.name, method.name);
                        parameters(state, &qualified, &method.params);
                        if method.body.is_some() && method.return_type.is_none() {
                            state.errors.push(TypeError::UnsupportedFeature {
                                feature: format!(
                                    "trait default method '{}' without an explicit return type",
                                    qualified
                                ),
                            });
                        }
                    }
                }
                Item::TypeAlias(alias) => insert(state, "type", "type", &alias.name),
                Item::Impl(block) => {
                    let target = direct_named_type(&block.target_type).unwrap_or("<invalid>");
                    if let Some(trait_name) = &block.trait_name {
                        let implementation = format!("{} for {}", trait_name, target);
                        if !state.impls.insert(implementation.clone()) {
                            state.errors.push(TypeError::DuplicateDeclaration {
                                kind: "impl".into(),
                                name: implementation,
                            });
                        }
                    }
                    let mut methods = HashSet::new();
                    for method in &block.methods {
                        if !methods.insert(method.name.as_str()) {
                            state.errors.push(TypeError::DuplicateDeclaration {
                                kind: "method".into(),
                                name: format!("{}::{}", target, method.name),
                            });
                        }
                        let qualified = format!("{}::{}", target, method.name);
                        parameters(state, &qualified, &method.params);
                        reject_unimplemented_function(state, &qualified, method);
                    }
                }
                Item::Module(module) => visit(state, &module.items),
                Item::Import(_) => {}
            }
        }
    }

    let values = base_functions
        .keys()
        .filter(|name| !name.contains("::"))
        .cloned()
        .collect();
    let mut state = State {
        values,
        types: builtin_type_names(base_functions),
        impls: HashSet::new(),
        errors: Vec::new(),
    };
    visit(&mut state, items);
    state.errors
}

fn builtin_type_names(base_functions: &HashMap<String, FunctionSig>) -> HashSet<String> {
    fn collect(ty: &Type, names: &mut HashSet<String>) {
        match ty {
            Type::Named(name) => {
                names.insert(name.clone());
            }
            Type::Array(inner) => collect(inner, names),
            Type::Tuple(items) => {
                for item in items {
                    collect(item, names);
                }
            }
            Type::Function(params, result) => {
                for param in params {
                    collect(param, names);
                }
                collect(result, names);
            }
            _ => {}
        }
    }

    let mut names: HashSet<String> = [
        "int", "i32", "i64", "u64", "usize", "float", "f32", "f64", "bool", "string", "str",
        "char", "Array", "Vec", "array", "map", "any", "Option", "Result",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    for signature in base_functions.values() {
        for param in &signature.params {
            collect(param, &mut names);
        }
        collect(&signature.result, &mut names);
    }
    names
}

fn collect_declared_type_names(items: &[Item], names: &mut HashSet<String>) {
    for item in items {
        match item {
            Item::Struct(structure) => {
                names.insert(structure.name.clone());
            }
            Item::Enum(enumeration) => {
                names.insert(enumeration.name.clone());
            }
            Item::TypeAlias(alias) => {
                names.insert(alias.name.clone());
            }
            Item::Module(module) => collect_declared_type_names(&module.items, names),
            _ => {}
        }
    }
}

fn declared_type_errors(items: &[Item], known: &HashSet<String>) -> Vec<TypeError> {
    #[derive(Default)]
    struct Issues {
        unknown: HashSet<String>,
        invalid_arguments: HashSet<(String, usize, usize)>,
        unsupported: HashSet<String>,
    }

    fn type_expr(ty: &TypeExpr, known: &HashSet<String>, issues: &mut Issues) {
        match ty {
            TypeExpr::Named { name, generics } => {
                if known.contains(name) {
                    // Titan currently has one real parameterized type shape:
                    // Array<T>/Vec<T>. Other named types are monomorphic. In
                    // particular, accepting and then discarding arguments on
                    // `Option<T>`, `map<K, V>`, primitives, or user types would
                    // advertise generic guarantees the checker does not keep.
                    let expected = usize::from(matches!(name.as_str(), "Array" | "Vec"));
                    if generics.len() != expected {
                        issues
                            .invalid_arguments
                            .insert((name.clone(), expected, generics.len()));
                    }
                } else {
                    issues.unknown.insert(name.clone());
                }
                for generic in generics {
                    type_expr(generic, known, issues);
                }
            }
            TypeExpr::Reference { inner, .. } => {
                issues.unsupported.insert("reference types".into());
                type_expr(inner, known, issues);
            }
            TypeExpr::Slice { inner } => type_expr(inner, known, issues),
            TypeExpr::Array { inner, size } => {
                issues.unsupported.insert("fixed-size array types".into());
                type_expr(inner, known, issues);
                expr(size, known, issues);
            }
            TypeExpr::Tuple { elements } => {
                for element in elements {
                    type_expr(element, known, issues);
                }
            }
            TypeExpr::Function {
                params,
                return_type,
            } => {
                for param in params {
                    type_expr(param, known, issues);
                }
                type_expr(return_type, known, issues);
            }
            TypeExpr::Unit | TypeExpr::Never | TypeExpr::Infer(_) => {}
        }
    }

    fn params(params: &[Param], known: &HashSet<String>, issues: &mut Issues) {
        for param in params {
            if let Some(annotation) = &param.type_ann {
                type_expr(annotation, known, issues);
            }
            if let Some(default) = &param.default {
                expr(default, known, issues);
            }
        }
    }

    fn pattern(current: &Pattern, known: &HashSet<String>, issues: &mut Issues) {
        match current {
            Pattern::Literal { value, .. } => expr(value, known, issues),
            Pattern::Struct { fields, .. } => {
                for (_, field) in fields {
                    pattern(field, known, issues);
                }
            }
            Pattern::Enum {
                inner: Some(inner), ..
            } => pattern(inner, known, issues),
            Pattern::Tuple { elements, .. } => {
                for element in elements {
                    pattern(element, known, issues);
                }
            }
            Pattern::Or { left, right, .. } => {
                pattern(left, known, issues);
                pattern(right, known, issues);
            }
            Pattern::Wildcard { .. }
            | Pattern::Ident { .. }
            | Pattern::Enum { inner: None, .. } => {}
        }
    }

    fn block(block: &Block, known: &HashSet<String>, issues: &mut Issues) {
        for statement in &block.stmts {
            match statement {
                Stmt::Expr(expression) => expr(expression, known, issues),
                Stmt::Let {
                    type_ann, value, ..
                } => {
                    if let Some(annotation) = type_ann {
                        type_expr(annotation, known, issues);
                    }
                    expr(value, known, issues);
                }
                Stmt::Assign { target, value, .. } => {
                    expr(target, known, issues);
                    expr(value, known, issues);
                }
                Stmt::Item(item) => visit_item(item, known, issues),
            }
        }
        if let Some(final_expression) = &block.final_expr {
            expr(final_expression, known, issues);
        }
    }

    fn expr(expression: &Expr, known: &HashSet<String>, issues: &mut Issues) {
        match expression {
            Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
                for element in elements {
                    expr(element, known, issues);
                }
            }
            Expr::StructLit { fields, .. } => {
                for (_, value) in fields {
                    expr(value, known, issues);
                }
            }
            Expr::Binary { left, right, .. }
            | Expr::Range {
                start: left,
                end: right,
                ..
            }
            | Expr::Index {
                target: left,
                index: right,
                ..
            }
            | Expr::Assign {
                target: left,
                value: right,
                ..
            } => {
                expr(left, known, issues);
                expr(right, known, issues);
            }
            Expr::Unary {
                expr: inner_expr, ..
            }
            | Expr::FieldAccess {
                target: inner_expr, ..
            }
            | Expr::Spawn {
                expr: inner_expr, ..
            }
            | Expr::Try {
                expr: inner_expr, ..
            } => expr(inner_expr, known, issues),
            Expr::Call { callee, args, .. } => {
                expr(callee, known, issues);
                for argument in args {
                    expr(argument, known, issues);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                expr(receiver, known, issues);
                for argument in args {
                    expr(argument, known, issues);
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                expr(condition, known, issues);
                block(then_branch, known, issues);
                if let Some(other) = else_branch {
                    block(other, known, issues);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                expr(scrutinee, known, issues);
                for arm in arms {
                    pattern(&arm.pattern, known, issues);
                    if let Some(guard) = &arm.guard {
                        expr(guard, known, issues);
                    }
                    block(&arm.body, known, issues);
                }
            }
            Expr::For {
                pattern: item_pattern,
                iterator,
                body,
                ..
            } => {
                pattern(item_pattern, known, issues);
                expr(iterator, known, issues);
                block(body, known, issues);
            }
            Expr::While {
                condition, body, ..
            } => {
                expr(condition, known, issues);
                block(body, known, issues);
            }
            Expr::Loop { body, .. } => block(body, known, issues),
            Expr::Block(body) => block(body, known, issues),
            Expr::Break { value, .. } | Expr::Return { value, .. } => {
                if let Some(value) = value {
                    expr(value, known, issues);
                }
            }
            Expr::Let {
                type_ann, value, ..
            } => {
                if let Some(annotation) = type_ann {
                    type_expr(annotation, known, issues);
                }
                expr(value, known, issues);
            }
            Expr::Closure {
                params: closure_params,
                return_type,
                body,
                ..
            } => {
                params(closure_params, known, issues);
                if let Some(result) = return_type {
                    type_expr(result, known, issues);
                }
                expr(body, known, issues);
            }
            Expr::Int { .. }
            | Expr::Float { .. }
            | Expr::String { .. }
            | Expr::StringTemplate { .. }
            | Expr::Char { .. }
            | Expr::Bool { .. }
            | Expr::Nil { .. }
            | Expr::Ident { .. }
            | Expr::Continue { .. } => {}
        }
    }

    fn visit_item(item: &Item, known: &HashSet<String>, issues: &mut Issues) {
        match item {
            Item::Function(function) => {
                params(&function.params, known, issues);
                if let Some(result) = &function.return_type {
                    type_expr(result, known, issues);
                }
                if let Some(body) = &function.body {
                    block(body, known, issues);
                }
            }
            Item::Struct(structure) => {
                for field in &structure.fields {
                    type_expr(&field.type_ann, known, issues);
                }
            }
            Item::Enum(enumeration) => {
                for variant in &enumeration.variants {
                    if let Some(payload) = &variant.payload {
                        type_expr(payload, known, issues);
                    }
                }
            }
            Item::Trait(trait_decl) => {
                for method in &trait_decl.methods {
                    params(&method.params, known, issues);
                    if let Some(result) = &method.return_type {
                        type_expr(result, known, issues);
                    }
                    if let Some(body) = &method.body {
                        block(body, known, issues);
                    }
                }
            }
            Item::Impl(implementation) => {
                type_expr(&implementation.target_type, known, issues);
                for method in &implementation.methods {
                    params(&method.params, known, issues);
                    if let Some(result) = &method.return_type {
                        type_expr(result, known, issues);
                    }
                    if let Some(body) = &method.body {
                        block(body, known, issues);
                    }
                }
            }
            Item::Const(constant) => {
                if let Some(annotation) = &constant.type_ann {
                    type_expr(annotation, known, issues);
                }
                expr(&constant.value, known, issues);
            }
            Item::TypeAlias(alias) => type_expr(&alias.target, known, issues),
            Item::Module(module) => {
                for item in &module.items {
                    visit_item(item, known, issues);
                }
            }
            Item::Import(_) => {}
        }
    }

    let mut issues = Issues::default();
    for item in items {
        visit_item(item, known, &mut issues);
    }

    let mut unknown: Vec<_> = issues.unknown.into_iter().collect();
    unknown.sort();
    let mut invalid_arguments: Vec<_> = issues.invalid_arguments.into_iter().collect();
    invalid_arguments.sort();
    let mut unsupported: Vec<_> = issues.unsupported.into_iter().collect();
    unsupported.sort();

    unknown
        .into_iter()
        .map(|name| TypeError::UnknownType { name })
        .chain(
            invalid_arguments
                .into_iter()
                .map(|(name, expected, found)| TypeError::InvalidTypeArguments {
                    name,
                    expected,
                    found,
                }),
        )
        .chain(
            unsupported
                .into_iter()
                .map(|feature| TypeError::UnsupportedFeature { feature }),
        )
        .collect()
}

fn alias_reaches(
    origin: &str,
    ty: &Type,
    aliases: &HashMap<String, Type>,
    visited: &mut HashSet<String>,
) -> bool {
    match ty {
        Type::Named(name) => {
            if name == origin {
                return true;
            }
            let Some(target) = aliases.get(name) else {
                return false;
            };
            if !visited.insert(name.clone()) {
                return false;
            }
            let recursive = alias_reaches(origin, target, aliases, visited);
            visited.remove(name);
            recursive
        }
        Type::Array(inner) => alias_reaches(origin, inner, aliases, visited),
        Type::Tuple(items) => items
            .iter()
            .any(|item| alias_reaches(origin, item, aliases, visited)),
        Type::Function(params, result) => {
            params
                .iter()
                .any(|param| alias_reaches(origin, param, aliases, visited))
                || alias_reaches(origin, result, aliases, visited)
        }
        _ => false,
    }
}

fn direct_named_type(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Named { name, generics } if generics.is_empty() => Some(name),
        _ => None,
    }
}

fn function_signature(function: &FunctionDecl, self_type: Option<&str>) -> FunctionSig {
    FunctionSig {
        params: function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                if index == 0 && param.name == "self" && param.type_ann.is_none() {
                    if let Some(self_type) = self_type {
                        return Type::Named(self_type.into());
                    }
                }
                param
                    .type_ann
                    .as_ref()
                    .map(type_from_ast)
                    .unwrap_or(Type::Unknown)
            })
            .collect(),
        result: function
            .return_type
            .as_ref()
            .map(type_from_ast)
            .unwrap_or(Type::Unit),
    }
}

fn trait_method_signature(method: &TraitMethod, self_type: &str) -> FunctionSig {
    FunctionSig {
        params: method
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                if index == 0 && param.name == "self" && param.type_ann.is_none() {
                    return Type::Named(self_type.into());
                }
                param
                    .type_ann
                    .as_ref()
                    .map(type_from_ast)
                    .unwrap_or(Type::Unknown)
            })
            .collect(),
        result: method
            .return_type
            .as_ref()
            .map(type_from_ast)
            .unwrap_or(Type::Unit),
    }
}

fn pattern_binding_names(pattern: &Pattern) -> HashSet<String> {
    let mut names = HashSet::new();
    match pattern {
        Pattern::Ident { name, .. } => {
            names.insert(name.clone());
        }
        Pattern::Enum {
            inner: Some(inner), ..
        } => names.extend(pattern_binding_names(inner)),
        Pattern::Tuple { elements, .. } => {
            for element in elements {
                names.extend(pattern_binding_names(element));
            }
        }
        Pattern::Struct { fields, .. } => {
            for (_, pattern) in fields {
                names.extend(pattern_binding_names(pattern));
            }
        }
        Pattern::Or { left, right, .. } => {
            names.extend(pattern_binding_names(left));
            names.extend(pattern_binding_names(right));
        }
        Pattern::Wildcard { .. } | Pattern::Literal { .. } | Pattern::Enum { inner: None, .. } => {}
    }
    names
}

fn match_pattern_is_lowerable(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard { .. } | Pattern::Ident { .. } | Pattern::Literal { .. } => true,
        Pattern::Enum { inner: None, .. } => true,
        Pattern::Enum {
            inner: Some(inner), ..
        } => matches!(
            inner.as_ref(),
            Pattern::Wildcard { .. } | Pattern::Ident { .. }
        ),
        Pattern::Or { .. } | Pattern::Tuple { .. } | Pattern::Struct { .. } => false,
    }
}

fn pattern_is_catchall(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard { .. } | Pattern::Ident { .. } => true,
        Pattern::Or { left, right, .. } => pattern_is_catchall(left) || pattern_is_catchall(right),
        _ => false,
    }
}

fn common_type(types: &[Type]) -> Option<Type> {
    let first = types.first()?;
    types
        .iter()
        .skip(1)
        .all(|candidate| compatible(first, candidate))
        .then(|| first.clone())
}

fn sequence_item_type(ty: &Type) -> Option<Type> {
    match ty {
        Type::Array(item) => Some(*item.clone()),
        Type::Tuple(items) => Some(common_type(items).unwrap_or(Type::Unknown)),
        Type::Unknown => Some(Type::Unknown),
        _ => None,
    }
}

/// Dedicated VM collection arguments accept both Value::Array and
/// Value::Tuple through `array_value`. Check every statically known tuple item
/// rather than collapsing a mixed tuple to Unknown and losing evidence.
fn sequence_matches(expected_item: &Type, found: &Type) -> bool {
    match found {
        Type::Array(item) => compatible(expected_item, item),
        Type::Tuple(items) => items
            .iter()
            .all(|item| compatible(expected_item, item)),
        Type::Unknown | Type::Never => true,
        _ => false,
    }
}

fn is_length_supported(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Array(_) | Type::Tuple(_) | Type::String | Type::Unknown
    ) || matches!(ty, Type::Named(name) if name == "bytes" || name == "map")
}

fn builtin_method_arity(receiver: &Type, method: &str) -> Option<usize> {
    match receiver {
        Type::Array(_) => match method {
            "len" => Some(0),
            "map" | "filter" | "sort_by" | "find" | "any" | "all" => Some(1),
            "fold" => Some(2),
            _ => None,
        },
        Type::Tuple(_) => match method {
            "len" => Some(0),
            "map" | "filter" | "sort_by" | "find" | "any" | "all" => Some(1),
            "fold" => Some(2),
            _ => None,
        },
        Type::String => (method == "len").then_some(0),
        Type::Named(name) if name == "bytes" || name == "map" => (method == "len").then_some(0),
        _ => None,
    }
}

fn lazy_rhs_is_skipped(operator: BinaryOp, left: &Expr) -> bool {
    matches!(
        (operator, left),
        (BinaryOp::LazyAnd, Expr::Bool { value: false, .. })
            | (BinaryOp::LazyOr, Expr::Bool { value: true, .. })
    )
}

fn lazy_rhs_is_guaranteed(operator: BinaryOp, left: &Expr) -> bool {
    matches!(
        (operator, left),
        (BinaryOp::LazyAnd, Expr::Bool { value: true, .. })
            | (BinaryOp::LazyOr, Expr::Bool { value: false, .. })
    )
}

fn block_may_break_current_loop(block: &Block) -> bool {
    block.stmts.iter().any(|statement| match statement {
        Stmt::Expr(expression) => expr_may_break_current_loop(expression),
        Stmt::Let { value, .. } => expr_may_break_current_loop(value),
        Stmt::Assign { target, value, .. } => {
            expr_may_break_current_loop(target) || expr_may_break_current_loop(value)
        }
        Stmt::Item(_) => false,
    }) || block
        .final_expr
        .as_deref()
        .is_some_and(expr_may_break_current_loop)
}

fn expr_may_break_current_loop(expression: &Expr) -> bool {
    match expression {
        Expr::Break { .. } => true,
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            elements.iter().any(expr_may_break_current_loop)
        }
        Expr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_may_break_current_loop(value)),
        Expr::Binary {
            left, op, right, ..
        } => {
            expr_may_break_current_loop(left)
                || (!lazy_rhs_is_skipped(*op, left) && expr_may_break_current_loop(right))
        }
        Expr::Range { start, end, .. } => {
            expr_may_break_current_loop(start) || expr_may_break_current_loop(end)
        }
        Expr::Unary { expr, .. }
        | Expr::FieldAccess { target: expr, .. }
        | Expr::Spawn { expr, .. }
        | Expr::Try { expr, .. } => expr_may_break_current_loop(expr),
        Expr::Call { callee, args, .. } => {
            expr_may_break_current_loop(callee) || args.iter().any(expr_may_break_current_loop)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_may_break_current_loop(receiver) || args.iter().any(expr_may_break_current_loop)
        }
        Expr::Index { target, index, .. } => {
            expr_may_break_current_loop(target) || expr_may_break_current_loop(index)
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            expr_may_break_current_loop(condition)
                || match condition.as_ref() {
                    Expr::Bool { value: true, .. } => block_may_break_current_loop(then_branch),
                    Expr::Bool { value: false, .. } => else_branch
                        .as_ref()
                        .is_some_and(block_may_break_current_loop),
                    _ => {
                        block_may_break_current_loop(then_branch)
                            || else_branch
                                .as_ref()
                                .is_some_and(block_may_break_current_loop)
                    }
                }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            expr_may_break_current_loop(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_deref()
                        .is_some_and(expr_may_break_current_loop)
                        || block_may_break_current_loop(&arm.body)
                })
        }
        // A break in the iterable/condition executes before entering the
        // nested loop. Breaks in its body belong to that nested loop.
        Expr::For { iterator, .. } => expr_may_break_current_loop(iterator),
        Expr::While { condition, .. } => expr_may_break_current_loop(condition),
        Expr::Loop { .. } | Expr::Closure { .. } => false,
        Expr::Return { value, .. } => value.as_deref().is_some_and(expr_may_break_current_loop),
        Expr::Let { value, .. } => expr_may_break_current_loop(value),
        Expr::Assign { target, value, .. } => {
            expr_may_break_current_loop(target) || expr_may_break_current_loop(value)
        }
        Expr::Block(block) => block_may_break_current_loop(block),
        Expr::Int { .. }
        | Expr::Float { .. }
        | Expr::String { .. }
        | Expr::StringTemplate { .. }
        | Expr::Char { .. }
        | Expr::Bool { .. }
        | Expr::Nil { .. }
        | Expr::Ident { .. }
        | Expr::Continue { .. } => false,
    }
}

fn block_definitely_returns(block: &Block) -> bool {
    for statement in &block.stmts {
        let exits = match statement {
            Stmt::Expr(expression) => expr_definitely_returns(expression),
            Stmt::Let { value, .. } => expr_definitely_returns(value),
            Stmt::Assign { target, value, .. } => {
                expr_definitely_returns(target) || expr_definitely_returns(value)
            }
            Stmt::Item(_) => false,
        };
        if exits {
            return true;
        }
    }
    block
        .final_expr
        .as_deref()
        .is_some_and(expr_definitely_returns)
}

fn expr_definitely_returns(expr: &Expr) -> bool {
    match expr {
        Expr::Return { .. } => true,
        Expr::Array { elements, .. } | Expr::Tuple { elements, .. } => {
            elements.iter().any(expr_definitely_returns)
        }
        Expr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_definitely_returns(value)),
        Expr::Binary {
            left, op, right, ..
        } => {
            expr_definitely_returns(left)
                || ((!matches!(op, BinaryOp::LazyAnd | BinaryOp::LazyOr)
                    || lazy_rhs_is_guaranteed(*op, left))
                    && expr_definitely_returns(right))
        }
        Expr::Range { start, end, .. } => {
            expr_definitely_returns(start) || expr_definitely_returns(end)
        }
        Expr::Unary { expr, .. }
        | Expr::FieldAccess { target: expr, .. }
        | Expr::Spawn { expr, .. }
        | Expr::Try { expr, .. } => expr_definitely_returns(expr),
        Expr::Call { callee, args, .. } => {
            expr_definitely_returns(callee) || args.iter().any(expr_definitely_returns)
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_definitely_returns(receiver) || args.iter().any(expr_definitely_returns)
        }
        Expr::Index { target, index, .. } => {
            expr_definitely_returns(target) || expr_definitely_returns(index)
        }
        Expr::Block(block) => block_definitely_returns(block),
        Expr::If {
            condition,
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => {
            expr_definitely_returns(condition)
                || match condition.as_ref() {
                    Expr::Bool { value: true, .. } => block_definitely_returns(then_branch),
                    Expr::Bool { value: false, .. } => block_definitely_returns(else_branch),
                    _ => {
                        block_definitely_returns(then_branch)
                            && block_definitely_returns(else_branch)
                    }
                }
        }
        Expr::If {
            condition,
            then_branch,
            else_branch: None,
            ..
        } => {
            expr_definitely_returns(condition)
                || (matches!(condition.as_ref(), Expr::Bool { value: true, .. })
                    && block_definitely_returns(then_branch))
        }
        // Match checking rejects every non-exhaustive expression before this
        // control-flow pass runs, including guarded catch-alls. Therefore a
        // non-empty match exits exactly when every reachable arm exits; it
        // does not need a syntactic wildcard (booleans and enums are finite).
        Expr::Match {
            scrutinee, arms, ..
        } => {
            expr_definitely_returns(scrutinee)
                || (!arms.is_empty() && arms.iter().all(|arm| block_definitely_returns(&arm.body)))
        }
        Expr::Loop { body, .. } => !block_may_break_current_loop(body),
        Expr::For { iterator, .. } => expr_definitely_returns(iterator),
        Expr::While {
            condition, body, ..
        } => {
            expr_definitely_returns(condition)
                || (matches!(condition.as_ref(), Expr::Bool { value: true, .. })
                    && !block_may_break_current_loop(body))
        }
        Expr::Let { value, .. } => expr_definitely_returns(value),
        Expr::Assign { target, value, .. } => {
            expr_definitely_returns(target) || expr_definitely_returns(value)
        }
        Expr::Int { .. }
        | Expr::Float { .. }
        | Expr::String { .. }
        | Expr::StringTemplate { .. }
        | Expr::Char { .. }
        | Expr::Bool { .. }
        | Expr::Nil { .. }
        | Expr::Ident { .. }
        | Expr::Break { .. }
        | Expr::Continue { .. }
        | Expr::Closure { .. } => false,
    }
}

fn type_from_ast(ty: &TypeExpr) -> Type {
    match ty {
        TypeExpr::Named { name, generics } => match name.as_str() {
            "int" | "i32" | "i64" | "u64" | "usize" => Type::Int,
            "float" | "f32" | "f64" => Type::Float,
            "bool" => Type::Bool,
            "string" | "str" => Type::String,
            "char" => Type::Char,
            "Array" | "Vec" if !generics.is_empty() => {
                Type::Array(Box::new(type_from_ast(&generics[0])))
            }
            // v0.16.0 QoL: bare `array` / `map` / `any` as convenient
            // aliases so users don't have to write `-> [any]` or
            // `Vec<Unknown>` for return types. Compatible with any
            // concrete Array(T) via the Unknown-matches-anything rule.
            "array" => Type::Array(Box::new(Type::Unknown)),
            "map" => Type::Named("map".into()),
            "any" => Type::Unknown,
            _ => Type::Named(name.clone()),
        },
        TypeExpr::Slice { inner } | TypeExpr::Array { inner, .. } => {
            Type::Array(Box::new(type_from_ast(inner)))
        }
        TypeExpr::Tuple { elements } => Type::Tuple(elements.iter().map(type_from_ast).collect()),
        TypeExpr::Function {
            params,
            return_type,
        } => Type::Function(
            params.iter().map(type_from_ast).collect(),
            Box::new(type_from_ast(return_type)),
        ),
        TypeExpr::Unit => Type::Unit,
        TypeExpr::Never => Type::Never,
        TypeExpr::Infer(_) => Type::Unknown,
        TypeExpr::Reference { inner, .. } => type_from_ast(inner),
    }
}
fn native_type(ty: titan_stdlib::native::NativeType) -> Type {
    use titan_stdlib::native::NativeType;
    match ty {
        NativeType::Any => Type::Unknown,
        NativeType::Int => Type::Int,
        NativeType::Float => Type::Float,
        NativeType::Bool => Type::Bool,
        NativeType::String => Type::String,
        NativeType::Bytes => Type::Named("bytes".into()),
        NativeType::Array => Type::Array(Box::new(Type::Unknown)),
        NativeType::Map => Type::Named("map".into()),
        NativeType::Option => Type::Named("Option".into()),
        NativeType::Nil => Type::Nil,
    }
}
fn native_compatible(expected: &Type, found: &Type) -> bool {
    if compatible(expected, found) || (expected == &Type::Float && found == &Type::Int) {
        return true;
    }
    match (expected, found) {
        // VM native byte readers intentionally accept strings by encoding
        // their UTF-8 bytes. Keep this directional: a native requiring a
        // string must not receive arbitrary bytes.
        (Type::Named(name), Type::String) if name == "bytes" => true,
        (Type::Array(expected), Type::Array(found)) => native_compatible(expected, found),
        (Type::Array(expected), Type::Tuple(found)) => {
            found.iter().all(|item| native_compatible(expected, item))
        }
        _ => false,
    }
}
/// Returns whether a function parameter can safely accept every value that a
/// caller of the expected function contract may provide. Function parameters
/// are contravariant: accepting `any` is valid where callers only send `int`,
/// but accepting only `int` is not valid where callers may send `any`.
fn function_parameter_accepts(parameter: &Type, argument: &Type) -> bool {
    if parameter == argument || argument == &Type::Never || parameter == &Type::Unknown {
        return true;
    }
    if argument == &Type::Unknown || parameter == &Type::Never {
        return false;
    }
    if matches!(
        (parameter, argument),
        (Type::Unit, Type::Nil) | (Type::Nil, Type::Unit)
    ) {
        return true;
    }
    match (parameter, argument) {
        (Type::Array(parameter), Type::Array(argument)) => {
            function_parameter_accepts(parameter, argument)
        }
        (Type::Tuple(parameters), Type::Tuple(arguments)) => {
            parameters.len() == arguments.len()
                && parameters
                    .iter()
                    .zip(arguments)
                    .all(|(parameter, argument)| function_parameter_accepts(parameter, argument))
        }
        (
            Type::Function(parameter_params, parameter_result),
            Type::Function(argument_params, argument_result),
        ) => function_type_compatible(
            parameter_params,
            parameter_result,
            argument_params,
            argument_result,
        ),
        _ => false,
    }
}

/// Function results are covariant while preserving the strict boundary around
/// `any`: a concrete result can satisfy `any`, but an unproven dynamic result
/// cannot satisfy a concrete function contract. `!` is safe for every result
/// because it never produces an incompatible value.
fn function_result_compatible(expected: &Type, found: &Type) -> bool {
    if expected == found || found == &Type::Never || expected == &Type::Unknown {
        return true;
    }
    if found == &Type::Unknown || expected == &Type::Never {
        return false;
    }
    if matches!(
        (expected, found),
        (Type::Unit, Type::Nil) | (Type::Nil, Type::Unit)
    ) {
        return true;
    }
    match (expected, found) {
        (Type::Array(expected), Type::Array(found)) => function_result_compatible(expected, found),
        (Type::Tuple(expected), Type::Tuple(found)) => {
            expected.len() == found.len()
                && expected
                    .iter()
                    .zip(found)
                    .all(|(expected, found)| function_result_compatible(expected, found))
        }
        (
            Type::Function(expected_params, expected_result),
            Type::Function(found_params, found_result),
        ) => function_type_compatible(expected_params, expected_result, found_params, found_result),
        _ => false,
    }
}

fn function_type_compatible(
    expected_params: &[Type],
    expected_result: &Type,
    found_params: &[Type],
    found_result: &Type,
) -> bool {
    expected_params.len() == found_params.len()
        && found_params
            .iter()
            .zip(expected_params)
            .all(|(parameter, argument)| function_parameter_accepts(parameter, argument))
        && function_result_compatible(expected_result, found_result)
}

fn compatible(a: &Type, b: &Type) -> bool {
    // Compatibility is directional. Never is the bottom type: an expression
    // that never completes can satisfy any expected type, but a concrete value
    // cannot satisfy an explicit `!` contract. Unknown remains the gradual
    // escape hatch in either direction, except that it cannot prove `!`.
    if a == b {
        return true;
    }
    if a == &Type::Never {
        return false;
    }
    if a == &Type::Unknown {
        return true;
    }
    if matches!(b, Type::Unknown | Type::Never) {
        return true;
    }
    if matches!((a, b), (Type::Unit, Type::Nil) | (Type::Nil, Type::Unit)) {
        return true;
    }
    // v0.16.0 QoL: recursively unify inside container types so
    // `-> array` (= Array(Unknown)) accepts a concrete Array(Int),
    // Array(Float), Array(Named("map")), etc. Similarly Tuple(Unknown)
    // matches any tuple with a compatible arity.
    match (a, b) {
        (Type::Array(x), Type::Array(y)) => compatible(x, y),
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(x, y)| compatible(x, y))
        }
        // Function components are deliberately stricter than ordinary dynamic
        // values. An already-inferred `fn(any) -> any` cannot safely become
        // `fn(string) -> int`; closures receive concrete context before their
        // body is checked instead.
        (
            Type::Function(expected_params, expected_result),
            Type::Function(found_params, found_result),
        ) => function_type_compatible(expected_params, expected_result, found_params, found_result),
        _ => false,
    }
}
fn is_numeric(ty: &Type) -> bool {
    matches!(ty, Type::Int | Type::Float | Type::Unknown)
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use titan_lexer::Lexer;
    use titan_parser::Parser;

    fn parse(source: &str) -> Program {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().0.to_vec();
        Parser::new(tokens).parse_program().unwrap()
    }

    fn check(source: &str) -> Result<(), Vec<TypeError>> {
        TypeEnv::new().check_program(&parse(source))
    }

    #[test]
    fn all_top_level_examples_typecheck() {
        let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut paths: Vec<_> = std::fs::read_dir(examples)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("titan"))
            .collect();
        paths.sort();
        let mut failures = Vec::new();
        for path in paths {
            let project = match titan_pkg::SourceProject::load(&path) {
                Ok(project) => project,
                Err(error) => {
                    failures.push(format!("{}: project loader: {error}", path.display()));
                    continue;
                }
            };
            if let Err(errors) = TypeEnv::new().check_program(&project.program) {
                failures.push(format!(
                    "{}: {}",
                    path.display(),
                    errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn accepts_recursive_typed_function() {
        assert!(
            check("fn fib(n: int) -> int { if n <= 1 { return n } fib(n-1) + fib(n-2) }").is_ok()
        );
    }
    #[test]
    fn rejects_unknown_names() {
        assert!(check("fn main() { missing + 1 }").is_err());
    }
    #[test]
    fn rejects_wrong_return() {
        assert!(check("fn bad() -> int { return true }").is_err());
    }
    #[test]
    fn checks_registered_native_signatures() {
        assert!(check("fn main() { std::text::reverse(\"Titan\") }").is_ok());
        assert!(check("fn main() { std::text::reverse(42) }").is_err());
        assert!(check("fn main() { std::encoding::hex_encode(\"Titan\") }").is_ok());
        assert!(check("fn main() { std::encoding::hex_encode(42) }").is_err());
    }

    #[test]
    fn resolves_aliases_at_native_call_boundaries() {
        assert!(check(
            "type Text = string fn main() { let value: Text = \"Titan\" std::text::reverse(value) }"
        )
        .is_ok());
        assert!(check(
            "type Payload = string fn main() { let value: Payload = \"Titan\" std::encoding::hex_encode(value) }"
        )
        .is_ok());
        assert!(check(
            "type Numbers = [int] fn main() { let values: Numbers = [1, 2, 3] std::stats::mean(values) }"
        )
        .is_ok());
    }
    #[test]
    fn generic_native_arrays_accept_concrete_elements() {
        assert!(check("fn main() { std::stats::mean([10, 20, 30, 40]) }").is_ok());
    }
    #[test]
    fn checks_tasks_and_channels() {
        assert!(check(
            "fn main() { let endpoints = channel(1) let task = spawn || 42 join(task) endpoints }"
        )
        .is_ok());
        assert!(check(
            "fn main() { let first = channel(1) let second = channel(1) select([first[1], second[1]], 10) select((first[1], second[1]), 10) }"
        )
        .is_ok());
        assert!(check(
            "type Inboxes = [Receiver] fn choose(inboxes: Inboxes) { select(inboxes, 10) } fn main() {}"
        )
        .is_ok());
        assert!(check("fn main() { spawn 42 }").is_err());
        assert!(check("fn main() { select(42, 10) }").is_err());
        assert!(
            check("fn main() { let endpoints = channel(1) select([endpoints[0]], 10) }")
                .is_err()
        );
        assert!(check("fn main() { select([\"not a receiver\"], 10) }").is_err());
        assert!(check(
            "fn main() { let select = |value: int, timeout: int| value + timeout select(42, 10) }"
        )
        .is_ok());
    }
    #[test]
    fn checks_tcp_handle_and_byte_signatures() {
        assert!(check("fn main() { let listener = std::net::tcp_listen(\"127.0.0.1:0\") let address = std::net::tcp_local_addr(listener) let stream = std::net::tcp_connect(address) let bytes = std::encoding::utf8_encode(\"ping\") std::net::tcp_write(stream, bytes) }").is_ok());
        assert!(check(
            "fn write(stream: TcpStream, tls: TlsStream) { std::net::tcp_write(stream, \"ping\") std::tls::write(tls, \"ping\") } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "type Text = string fn write(stream: TcpStream, value: Text) { std::net::tcp_write(stream, value) } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "type Socket = TcpStream type Listener = TcpListener fn close(stream: Socket, listener: Listener) { std::net::tcp_close(stream) std::net::tcp_close(listener) } fn main() {}"
        )
        .is_ok());
        assert!(check("fn main() { std::net::tcp_close(42) }").is_err());
        assert!(check("fn main() { std::net::tcp_close(\"not a handle\") }").is_err());
        assert!(check(
            "fn write(socket: WebSocket) { std::ws::send_binary(socket, \"not bytes\") } fn main() {}"
        )
        .is_err());
        assert!(check(
            "fn feed(decoder: WebSocketDecoder) { std::ws::decoder_push(decoder, \"not bytes\") } fn main() {}"
        )
        .is_err());
    }

    #[test]
    fn validates_dedicated_database_argument_shapes() {
        assert!(check(
            "type LocalDb = Sqlite fn use_all(local: LocalDb, postgres: Postgres, mysql: Mysql) { std::db::ping(local) std::db::begin(postgres) std::db::close(mysql) std::db::execute(local, \"SELECT 1\", []) std::db::query(postgres, \"SELECT 1\", (1, \"two\")) std::db::migrate(mysql, []) } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn use_specific(local: Sqlite, postgres: Postgres, mysql: Mysql) { std::sqlite::execute(local, \"SELECT 1\", []) std::postgres::query(postgres, \"SELECT 1\", (1, \"two\")) std::mysql::migrate(mysql, []) } fn main() {}"
        )
        .is_ok());
        assert!(check("fn main() { std::db::ping(42) }").is_err());
        assert!(check("fn main() { std::db::query(42, \"SELECT 1\", []) }").is_err());
        assert!(check(
            "fn query(local: Sqlite) { std::sqlite::execute(local, \"SELECT 1\", \"not parameters\") } fn main() {}"
        )
        .is_err());
        assert!(check(
            "fn migrate(local: Sqlite) { std::db::migrate(local, [1, 2]) } fn main() {}"
        )
        .is_err());
    }

    #[test]
    fn validates_vm_managed_callback_contracts() {
        assert!(check(
            "fn register(listener: TcpListener) { std::http::serve_connection(listener, |request| request, 1) } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn main() { let router = std::http::router() std::http::route(router, \"GET\", \"/\", |request| request.path) std::http::middleware(router, |request| request) std::http::after(router, |response| response) std::http::on_error(router, |request, error| request) }"
        )
        .is_ok());
        assert!(check(
            "fn register(listener: TcpListener) { std::http::serve_connection(listener, 42, 1) } fn main() {}"
        )
        .is_err());
        assert!(check(
            "fn main() { let router = std::http::router() std::http::route(router, \"GET\", \"/\", 42) }"
        )
        .is_err());
        assert!(check(
            "fn main() { let router = std::http::router() std::http::middleware(router, |request| 42) }"
        )
        .is_err());
        assert!(check(
            "fn main() { let router = std::http::router() std::http::dispatch(router, 42) }"
        )
        .is_err());
        assert!(check("fn main() { std::runtime::benchmark(1, || 42) }").is_ok());
        assert!(check("fn main() { std::runtime::benchmark(1, 42) }").is_err());
        assert!(check("fn main() { std::runtime::benchmark(1, |value| value) }").is_err());
        assert!(check("fn main() { std::runtime::spawn_quota(1024, || 42) }").is_ok());
        assert!(check("fn main() { std::runtime::spawn_quota(1024, 42) }").is_err());
    }

    #[test]
    fn rejects_parameter_defaults_until_the_runtime_can_apply_them() {
        assert!(check("fn bad(value: int = true) -> int { value }").is_err());
        assert!(check("fn bad(value: int = value) -> int { value }").is_err());
        assert!(check("fn pending(first: int, second: int = first) -> int { second }").is_err());
    }

    #[test]
    fn rejects_bodyless_and_unlinked_extern_functions() {
        assert!(check("fn declared() -> int; fn main() { declared() }").is_err());
        assert!(check("extern \"C\" fn native() -> int; fn main() { native() }").is_err());
    }

    #[test]
    fn annotated_constants_keep_the_declared_type() {
        assert!(check("const FLEXIBLE: any = 1 fn main() { FLEXIBLE[0] }").is_ok());
        assert!(check("const INVALID: int = true fn main() {}").is_err());
    }

    #[test]
    fn infers_global_constants_independent_of_order() {
        let forward =
            "fn main() { let value: int = FIRST } const FIRST = SECOND + 1 const SECOND = 41";
        assert!(check(forward).is_ok());
        assert!(check("const FIRST: string = SECOND const SECOND = 42 fn main() {}").is_err());
    }

    #[test]
    fn rejects_direct_and_indirect_constant_cycles_but_not_local_shadowing() {
        for source in [
            "const VALUE = VALUE fn main() {}",
            "const FIRST = SECOND const SECOND = FIRST fn main() {}",
            "const FIRST: int = SECOND const SECOND: int = FIRST fn main() {}",
        ] {
            let errors = check(source).unwrap_err();
            assert!(errors
                .iter()
                .any(|error| matches!(error, TypeError::RecursiveConstant { .. })));
        }
        assert!(check("const VALUE = (|VALUE| VALUE)(42) fn main() { VALUE }").is_ok());
    }

    #[test]
    fn validates_unary_and_binary_operator_domains() {
        assert!(check("fn main() { let a = -true let b = ~\"text\" }").is_err());
        assert!(check("fn main() { let a = -1.5 let b = ~7 }").is_ok());
        assert!(check("fn main() { 5.0 % 2.0 }").is_err());
        assert!(check("fn main() { [1, 2].filter(|value| value % 2 == 0) }").is_ok());
        assert!(check("fn bits(left: any, right: any) -> int { left & right } fn main() {}")
            .is_ok());
        assert!(check("fn main() { true | 1 }").is_err());
        assert!(check("fn unit() {} fn main() { nil == unit() unit() == nil }").is_ok());
    }

    #[test]
    fn validates_index_targets_and_tuple_indexes() {
        assert!(check("fn main() { let value = true[0] }").is_err());
        assert!(check("fn main() { let pair = (1, \"two\") let value: string = pair.1 }").is_ok());
        assert!(check("fn main() { let pair = (1, 2) pair.2 }").is_err());
    }

    #[test]
    fn validates_iterable_values() {
        assert!(check("fn main() { for value in true { print(value) } }").is_err());
        assert!(check("fn main() { for value in \"ok\" { print(value) } }").is_ok());
    }

    #[test]
    fn validates_user_method_arguments_and_unknown_methods() {
        let declarations = "struct Point { x: int } impl Point { fn add(self, amount: int) -> int { self.x + amount } }";
        assert!(check(&format!(
            "{declarations} fn main() {{ let point = Point {{ x: 1 }} point.add(2) }}"
        ))
        .is_ok());
        assert!(check(&format!(
            "{declarations} fn main() {{ let point = Point {{ x: 1 }} point.add(true) }}"
        ))
        .is_err());
        assert!(check(&format!(
            "{declarations} fn main() {{ let point = Point {{ x: 1 }} point.missing() }}"
        ))
        .is_err());
    }

    #[test]
    fn validates_collection_method_callbacks() {
        assert!(check("fn main() { [1, 2].map(|value: int| value + 1) }").is_ok());
        assert!(check("fn main() { [1, 2].filter(|value: int| value + 1) }").is_err());
        assert!(check("fn main() { [1, 2].filter(|value| value + 1) }").is_err());
        assert!(
            check("fn main() { let values: [string] = [1, 2].map(|value| value + 1) }").is_err()
        );
        assert!(check("fn main() { [1, 2].map(42) }").is_err());
        assert!(check("fn main() { [1, 2].map(|| 1) }").is_err());
        assert!(
            check("fn main() { let values: [int] = (1, 2).map(|value: int| value + 1) }").is_ok()
        );
    }

    #[test]
    fn validates_global_collection_intrinsics() {
        assert!(check("fn main() { len(42) }").is_err());
        assert!(check("fn main() { map(42, |value| value) }").is_err());
        assert!(check("fn main() { filter([1, 2], |value: int| value + 1) }").is_err());
        assert!(check("fn main() { filter([1, 2], |value| value + 1) }").is_err());
        assert!(
            check("fn main() { let values: [string] = map([1, 2], |value| value + 1) }").is_err()
        );
        assert!(
            check("fn main() { let values: [string] = map((1, 2), |value: int| \"ok\") }").is_ok()
        );
    }

    #[test]
    fn local_callables_shadow_collection_intrinsics() {
        assert!(check(
            "fn main() { let len: fn(string) -> string = |value| value let result: string = len(\"ok\") result }"
        )
        .is_ok());
        assert!(check(
            "fn main() { let map: fn(int, int) -> int = |left, right| left + right let result: int = map(20, 22) result }"
        )
        .is_ok());
        assert!(check(
            "fn main() { let map: fn(int, int) -> int = |left, right| left + right map(\"bad\", 2) }"
        )
        .is_err());
        assert!(check("fn main() { let print: fn(int) -> int = |value| value print(1) }").is_ok());
        assert!(check("fn main() { let print: fn(int) -> int = |value| value print() }").is_err());
        assert!(check("fn main() { print() print(1, 2, 3) }").is_ok());
    }

    #[test]
    fn contextualizes_closures_before_checking_their_bodies() {
        assert!(check(
            "fn main() { let transform: fn(int) -> int = |value| value + 1 transform(2) }"
        )
        .is_ok());
        assert!(check(
            "fn main() { let invalid: fn(string) -> int = |value| value * 2 invalid(\"x\") }"
        )
        .is_err());
        assert!(check(
            "fn apply(callback: fn(string) -> int) -> int { callback(\"x\") } fn main() { apply(|value| value * 2) }"
        )
        .is_err());
        assert!(check(
            "fn invalid_factory() -> fn(string) -> int { |value| value * 2 } fn main() {}"
        )
        .is_err());
        assert!(check(
            "fn main() { let loose = |value| value * 2 let invalid: fn(string) -> int = loose }"
        )
        .is_err());
        assert!(check(
            "fn apply(callback: fn(int) -> int) -> int { callback(1) } fn main() { let loose = |value| value * 2 apply(loose) }"
        )
        .is_err());
    }

    #[test]
    fn checks_function_parameter_and_result_variance() {
        assert!(check(
            "fn flexible(value: any) -> string { \"ok\" } fn apply(callback: fn(string) -> any) -> any { callback(\"x\") } fn main() { apply(flexible) }"
        )
        .is_ok());
        assert!(check(
            "fn narrow(value: string) -> int { 1 } fn apply(callback: fn(any) -> int) -> int { callback(1) } fn main() { apply(narrow) }"
        )
        .is_err());
        assert!(check(
            "fn dynamic() -> any { 1 } fn apply(callback: fn() -> int) -> int { callback() } fn main() { apply(dynamic) }"
        )
        .is_err());
        assert!(check(
            "fn keep(value: any) -> bool { true } fn main() { let values: [int] = filter([1], keep) }"
        )
        .is_ok());
        assert!(
            check("fn dynamic(value: int) -> any { true } fn main() { filter([1], dynamic) }")
                .is_err()
        );
    }

    #[test]
    fn preserves_mixed_array_evidence_across_inferred_bindings() {
        assert!(check("fn main() { let values: [int] = [1, \"bad\"] }").is_err());
        assert!(check("fn main() { let mixed = [1, \"bad\"] let values: [int] = mixed }").is_err());
        assert!(check(
            "fn main() { let mixed = [1, \"ok\"] let values: array = mixed print(values) }"
        )
        .is_ok());
        assert!(check(
            "fn take(values: [int]) {} fn main() { let mixed = [1, \"bad\"] take(mixed) }"
        )
        .is_err());
    }

    #[test]
    fn validates_string_template_references_and_calls() {
        assert!(check("fn main() { print(\"value={missing}\") }").is_err());
        assert!(check("fn id(value: int) -> int { value } fn main() { let value = 1 print(\"value={id(value)}\") }").is_ok());
        assert!(check("fn id(value: int) -> int { value } fn main() { let bad = \"x\" print(\"value={id(bad)}\") }").is_err());
        assert!(
            check("fn main() { let value = 1 print(\"ok={value}, bad={value + 1}\") }").is_err()
        );
        assert!(check(
            "fn callback() -> int { 1 } fn render(value: any) -> any { value } fn main() { print(\"value={render(callback)}\") }"
        )
        .is_err());
        assert!(
            check("fn callback() -> int { 1 } fn main() { print(\"value={callback}\") }").is_err()
        );
        assert!(check(
            "fn render(value: any) -> any { value } fn main() { let callback = || 1 print(\"value={render(callback)}\") }"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop {} } fn render() -> ! { \"value={halt()}\" } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop {} } fn inferred() { \"value={halt()}\" } fn main() { let impossible: ! = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop {} } fn main() { print(\"stop={halt()}, bad={missing}\") }"
        )
        .is_err());
    }

    #[test]
    fn validates_assignment_mutability_and_compound_types() {
        assert!(check("fn main() { let values = [1] values[0] = 2 }").is_err());
        assert!(check("fn main() { let value = 1 value = 2 }").is_err());
        assert!(check("fn main() { let mut value = \"text\" value -= 1 }").is_err());
        assert!(check("fn main() { let mut value = 1 value += 2 }").is_ok());
        assert!(check("fn bump(mut value: int) -> int { value += 1 value } fn main() {}").is_ok());
        assert!(check("fn bump(value: int) -> int { value += 1 value } fn main() {}").is_err());
        assert!(
            check("fn main() { let mut value = 1 let update = || { value = 2 } update() }")
                .is_err()
        );
        assert!(check("const VALUE: int = 1 fn main() { VALUE = 2 }").is_err());
    }

    #[test]
    fn validates_returns_and_spawned_closure_arity() {
        assert!(check("fn missing(flag: bool) -> int { if flag { return 1 } }").is_err());
        assert!(check("fn complete(flag: bool) -> int { if flag { return 1 } return 2 }").is_ok());
        assert!(check("fn branch(flag: bool) -> int { if flag { return 1 } else { 2 } }").is_ok());
        assert!(check("fn literal() -> int { if true { return 1 } }").is_ok());
        assert!(check("fn literal() -> int { if true { 1 } else { \"unreachable\" } }").is_ok());
        assert!(check("fn literal() -> int { if false { \"unreachable\" } else { 1 } }").is_ok());
        assert!(check("fn literal() -> int { if false { true + 1 } else { 1 } }").is_err());
        assert!(check("fn mixed(flag: bool) -> int { if flag { 1 } else { \"bad\" } }").is_err());
        assert!(check("fn bad_unit() -> () { 42 }").is_err());
        assert!(check("fn empty_unit() -> () {}").is_ok());
        assert!(check(
            "fn compatible_unit(flag: bool) -> () { if flag { print(\"ok\") } else { nil } }"
        )
        .is_ok());
        assert!(check("const INVALID = return 1 fn main() {}").is_err());
        assert!(check("fn main() { spawn |value: int| value }").is_err());
    }

    #[test]
    fn enforces_directional_never_and_infinite_loop_types() {
        assert!(check("fn invalid() -> ! { 1 } fn main() {}").is_err());
        assert!(check("fn invalid() -> ! { return 1 } fn main() {}").is_err());
        assert!(check("fn main() { let impossible: ! = 1 }").is_err());
        assert!(check("fn halt() -> ! { loop {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> int { loop {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> int { let never = loop {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { while true {} } fn main() {}").is_ok());
        assert!(
            check("fn exits(flag: bool) -> int { loop { if flag { break } } } fn main() {}")
                .is_err()
        );
        assert!(check("fn nested() -> ! { loop { loop { break } } } fn main() {}").is_ok());
    }

    #[test]
    fn propagates_never_through_eager_expression_evaluation() {
        assert!(check("fn halt() -> ! { [1, loop {}] } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { (1, loop {}) } fn main() {}").is_ok());
        assert!(check(
            "struct Item { value: int } fn halt() -> ! { Item { value: loop {} } } fn main() {}"
        )
        .is_ok());
        assert!(check("fn halt() -> ! { 1 + loop {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { -(loop {}) } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { 0..loop {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { [1][loop {}] } fn main() {}").is_ok());
        assert!(check(
            "fn consume(value: int) -> int { value } fn halt() -> ! { consume(loop {}) } fn main() {}"
        )
        .is_ok());
        assert!(check("fn halt() -> ! { (loop {})() } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { (loop {}).len() } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { (loop {})? } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { if loop {} { 1 } else { 2 } } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { match loop {} {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { for item in loop {} {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { while loop {} {} } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { let mut value = 0 value = loop {} } fn main() {}").is_ok());
        assert!(check(
            "fn inferred() { loop {} return 1 } fn main() { let impossible: ! = inferred() }"
        )
        .is_ok());
        assert!(check(
            "type Nope = ! fn halt() -> Nope { loop {} } fn main() { let impossible: ! = halt() }"
        )
        .is_ok());
        assert!(check(
            "fn sink(first: any, second: any) {} fn inferred() { sink(loop {}, return 1) } fn main() { let impossible: ! = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn sink(first: any, second: any) {} fn inferred() { sink(return 1, return \"unreachable\") } fn main() { let number: int = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { [loop {}, return 1] } fn main() { let impossible: ! = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { fold([1], loop {}, return 1) } fn main() { let impossible: ! = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { [1].fold(loop {}, return 1) } fn main() { let impossible: ! = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn work(first: any, second: any) -> int { 1 } fn inferred() { std::try::catch(work, loop {}, return 1) } fn main() { let impossible: ! = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop {} } fn invoke(callback: fn() -> int) -> int { callback() } fn main() { invoke(halt) }"
        )
        .is_ok());
        assert!(check(
            "fn value() -> int { 1 } fn require_never(callback: fn() -> !) { callback() } fn main() { require_never(value) }"
        )
        .is_err());
        assert!(check(
            "fn stop(value: int) -> ! { loop {} } fn main() { let values: [int] = filter([1], stop) }"
        )
        .is_ok());
        assert!(check("fn main() { let halt = || loop {} let impossible: ! = halt() }").is_ok());

        // A dynamic lazy RHS is conditional, while a literal can either skip
        // it entirely or guarantee that it runs.
        assert!(check(
            "fn value() -> bool { false && loop {} } fn main() { let result: bool = value() }"
        )
        .is_ok());
        assert!(check(
            "fn value() -> bool { true || loop {} } fn main() { let result: bool = value() }"
        )
        .is_ok());
        assert!(check(
            "fn halt_and() -> ! { true && loop {} } fn halt_or() -> ! { false || loop {} } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { false && return 1 true } fn main() { let result: bool = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { true || return 1 false } fn main() { let result: bool = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { if false { return \"unreachable\" } 1 } fn main() { let result: int = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { while false { return \"unreachable\" } 1 } fn main() { let result: int = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { if true { loop {} } else { 1 } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { if false { 1 } else { loop {} } } fn main() {}"
        )
        .is_ok());
    }

    #[test]
    fn ignores_unreachable_breaks_when_classifying_loops() {
        assert!(check(
            "fn inferred() { loop { return 1 break } } fn main() { let value: int = inferred() }"
        )
        .is_ok());
        assert!(check("fn value() -> int { while true { return 1 break } } fn main() {}").is_ok());
        assert!(check("fn halt() -> ! { loop { loop {} break } } fn main() {}").is_ok());
        assert!(check(
            "fn sink(first: any, second: any) {} fn halt() -> ! { loop { sink(loop {}, { break }) } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt_and() -> ! { loop { false && { break } } } fn halt_or() -> ! { loop { true || { break } } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop { if false { break } } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop { if true { loop {} } else { break } } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop { match true { true => { loop {} }, false => { break } } } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop { match true { true if false => { break }, true => { loop {} }, false => { break } } } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { loop { match 1 { 0 => { break }, _ => { loop {} } } } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "enum Choice { Stop, Continue } fn halt() -> ! { loop { match (Choice::Continue) { Choice::Stop => { break }, Choice::Continue => { loop {} } } } } fn main() {}"
        )
        .is_ok());
        assert!(check("fn completes() -> ! { loop { break } } fn main() {}").is_err());
        assert!(check("fn completes() -> ! { loop { if true { break } } } fn main() {}").is_err());
        assert!(check("fn completes() -> ! { loop { true && { break } } } fn main() {}").is_err());
        assert!(check("fn completes() -> ! { loop { false || { break } } } fn main() {}").is_err());
    }

    #[test]
    fn validates_try_operands_and_propagated_return_paths() {
        assert!(check(
            "fn good() -> Result { let value = Result::Ok(1)? Result::Ok(value) } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { let value = Result::Ok(1)? Result::Ok(value) } fn main() { inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn wrong_contract() -> int { let value = Result::Ok(1)? value } fn main() {}"
        )
        .is_err());
        assert!(check("fn dynamic(value: any) -> any { value? } fn main() {}").is_err());
        assert!(check("const INVALID = Result::Ok(1)? fn main() {}").is_err());
    }

    #[test]
    fn validates_try_catch_callable_and_invocation_shape() {
        assert!(check(
            "fn main() { let result = std::try::catch(|value| value + 1, 2) print(result) }"
        )
        .is_ok());
        assert!(check("fn main() { std::try::catch() }").is_err());
        assert!(check("fn main() { std::try::catch(42) }").is_err());
        assert!(check("fn main() { std::try::catch(|| 1, 2) }").is_err());
        assert!(check("fn main() { std::try::catch(|value: string| value, 2) }").is_err());
    }

    #[test]
    fn rejects_syntax_that_bytecode_cannot_lower() {
        assert!(check("fn main() { let value = 1; &value }").is_err());
        assert!(check("fn main() { loop { break 1 } }").is_err());
        assert!(check("fn main() { fn nested() {} }").is_err());
        assert!(check("fn main() { for _ in [1, 2] {} }").is_err());
        assert!(check("fn main() { let operation = len operation([1, 2]) }").is_err());
        assert!(check("fn main() { let operation = std::net::tcp_read operation }").is_err());
        assert!(check(
            "fn identity(value: int) -> int { value } fn main() { let operation = identity operation(1) }"
        )
        .is_ok());
        assert!(check(
            "fn read(value: bool) -> int { match value { true | false => 1 } } fn main() {}"
        )
        .is_err());
        assert!(check(
            "enum Inner { Yes } enum Outer { Wrap(Inner) } fn read(value: Outer) -> int { match value { Outer::Wrap(Inner::Yes) => 1, _ => 0 } } fn main() {}"
        )
        .is_err());
    }

    #[test]
    fn infers_unannotated_return_types_independent_of_order() {
        assert!(check("fn main() { let value: int = later() } fn later() { 42 }").is_ok());
        assert!(check(
            "fn mixed(flag: bool) { if flag { return 1 } \"text\" } fn main() { mixed(true) }"
        )
        .is_err());
        assert!(
            check("fn main() { let callback = || return 42 let value: int = callback() }").is_ok()
        );
    }

    #[test]
    fn reusable_environment_does_not_leak_user_declarations() {
        let mut environment = TypeEnv::new();
        assert!(environment
            .check_program(&parse("fn helper() -> int { 1 } fn main() { helper() }"))
            .is_ok());
        assert!(environment
            .check_program(&parse("fn main() { helper() }"))
            .is_err());
    }

    #[test]
    fn rejects_duplicate_declarations_and_members() {
        assert!(check("fn same() {} fn same() {} fn main() {}").is_err());
        assert!(check("struct Pair { left: int, left: int } fn main() {}").is_err());
        assert!(check("enum Choice { Yes, Yes } fn main() {}").is_err());
        assert!(check("fn duplicate(value: int, value: int) {} fn main() {}").is_err());
        assert!(check(
            "fn main() { let duplicate = |value: int, value: int| value duplicate(1, 2) }"
        )
        .is_err());
        assert!(check("struct Point { x: int } fn main() { Point { x: 1, x: 2 } }").is_err());
        assert!(check("fn main() { Missing { value: unknown } }").is_err());
    }

    #[test]
    fn validates_declared_type_names() {
        assert!(check("fn bad(value: Innt) {} fn main() {}").is_err());
        assert!(check("fn make() -> Later { Later { value: 1 } } struct Later { value: int } fn main() { make() }").is_ok());
        assert!(check(
            "fn use_database(database: Sqlite) { std::sqlite::ping(database) } fn main() {}"
        )
        .is_ok());

        let local_errors =
            check("fn main() { let value: Missing = 1 let callback = |item: AlsoMissing| item }")
                .unwrap_err();
        assert!(local_errors
            .iter()
            .any(|error| matches!(error, TypeError::UnknownType { name } if name == "Missing")));
        assert!(local_errors.iter().any(
            |error| matches!(error, TypeError::UnknownType { name } if name == "AlsoMissing")
        ));
    }

    #[test]
    fn rejects_unimplemented_type_shapes_and_generic_arguments() {
        assert!(check("fn consume(value: Array<int>) {} fn main() {}").is_ok());

        for (source, expected_name, expected, found) in [
            ("fn consume(value: Array) {} fn main() {}", "Array", 1, 0),
            (
                "fn consume(value: Array<int, string>) {} fn main() {}",
                "Array",
                1,
                2,
            ),
            (
                "fn consume(value: Option<int>) {} fn main() {}",
                "Option",
                0,
                1,
            ),
            (
                "fn consume(value: int<string>) {} fn main() {}",
                "int",
                0,
                1,
            ),
            (
                "fn consume(value: map<string, int>) {} fn main() {}",
                "map",
                0,
                2,
            ),
        ] {
            let errors = check(source).unwrap_err();
            assert!(errors.iter().any(|error| matches!(
                error,
                TypeError::InvalidTypeArguments { name, expected: actual_expected, found: actual_found }
                    if name == expected_name
                        && *actual_expected == expected
                        && *actual_found == found
            )));
        }

        for (source, expected_feature) in [
            ("fn consume(value: &int) {} fn main() {}", "reference types"),
            (
                "fn consume(value: [int; 3]) {} fn main() {}",
                "fixed-size array types",
            ),
        ] {
            let errors = check(source).unwrap_err();
            assert!(errors.iter().any(|error| matches!(
                error,
                TypeError::UnsupportedFeature { feature } if feature == expected_feature
            )));
        }
    }

    #[test]
    fn rejects_recursive_aliases_and_resolves_long_chains() {
        assert!(check("type Cycle = Cycle fn main() {}").is_err());
        assert!(check("type Cycle = [Cycle] fn main() {}").is_err());

        let mut source = String::new();
        for index in 0..24 {
            source.push_str(&format!("type Alias{index} = Alias{} ", index + 1));
        }
        source.push_str("type Alias24 = int fn identity(value: Alias0) -> int { value } fn main() { identity(1) }");
        assert!(check(&source).is_ok());
    }

    #[test]
    fn validates_trait_implementation_signatures() {
        let valid = "trait Measure { fn value(self, scale: int) -> int; } struct Item { value: int } impl Measure for Item { fn value(self, scale: int) -> int { self.value * scale } } fn main() { let item = Item { value: 2 } item.value(3) }";
        assert!(check(valid).is_ok());

        let wrong_type = "trait Measure { fn value(self, scale: int) -> int; } struct Item { value: int } impl Measure for Item { fn value(self, scale: string) -> int { 0 } } fn main() {}";
        assert!(check(wrong_type).is_err());

        let extra_method = "trait Measure { fn value(self) -> int; } struct Item { value: int } impl Measure for Item { fn value(self) -> int { self.value } fn extra(self) {} } fn main() {}";
        assert!(check(extra_method).is_err());

        let inferred = "trait Measure { fn value(self) -> int; } struct Item { value: int } impl Measure for Item { fn value(self) { self.value } } fn main() { let item = Item { value: 2 } let value: int = item.value() }";
        assert!(check(inferred).is_ok());

        let inferred_mismatch = "trait Measure { fn value(self) -> int; } struct Item { value: int } impl Measure for Item { fn value(self) { \"wrong\" } } fn main() {}";
        assert!(check(inferred_mismatch).is_err());
    }

    #[test]
    fn trait_defaults_require_an_explicit_contract_return_type() {
        let inferred_default = "trait Label { fn label(self) { \"label\" } } struct Item { value: int } impl Label for Item {} fn main() {}";
        assert!(check(inferred_default).is_err());

        let explicit_default = "trait Label { fn label(self) -> string { \"label\" } } struct Item { value: int } impl Label for Item {} fn main() { let item = Item { value: 1 } let label: string = item.label() }";
        assert!(check(explicit_default).is_ok());

        let unit_requirement = "trait Reset { fn reset(self); } struct Item { value: int } impl Reset for Item { fn reset(self) {} } fn main() {}";
        assert!(check(unit_requirement).is_ok());
    }

    #[test]
    fn rejects_invalid_impl_targets_and_method_collisions() {
        assert!(check(
            "type Number = int impl Number { fn value(self) -> int { self } } fn main() {}"
        )
        .is_err());
        let collision = "trait First { fn label(self) -> string { \"first\" } } trait Second { fn label(self) -> string { \"second\" } } struct Item { value: int } impl First for Item {} impl Second for Item {} fn main() {}";
        assert!(check(collision).is_err());
    }

    #[test]
    fn validates_enum_patterns_and_exhaustiveness() {
        let valid = "enum Choice { Number(int), Empty } fn read(choice: Choice) -> int { match choice { Choice::Number(value) => value, Choice::Empty => 0 } } fn main() {}";
        assert!(check(valid).is_ok());

        let missing = "enum Choice { Number(int), Empty } fn read(choice: Choice) -> int { match choice { Choice::Number(value) => value } } fn main() {}";
        assert!(check(missing).is_err());

        let unknown = "enum Choice { Empty } fn read(choice: Choice) -> int { match choice { Choice::Missing => 1, _ => 0 } } fn main() {}";
        assert!(check(unknown).is_err());

        let invalid_payload = "enum Choice { Empty } fn read(choice: Choice) -> int { match choice { Choice::Empty(value) => value, _ => 0 } } fn main() {}";
        assert!(check(invalid_payload).is_err());

        let missing_payload = "enum Choice { Number(int), Empty } fn read(choice: Choice) -> int { match choice { Choice::Number => 1, Choice::Empty => 0 } } fn main() {}";
        assert!(check(missing_payload).is_err());
    }

    #[test]
    fn match_result_ignores_branches_that_never_continue() {
        assert!(check(
            "fn read(flag: bool) -> int { let value = match flag { true => return 1, false => 2 } value + 1 } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn read(flag: bool) -> int { match flag { true => 1, false => \"bad\" } } fn main() {}"
        )
        .is_err());
        assert!(check(
            "fn read() -> int { match true { true => 1, false => \"unreachable\" } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn halt() -> ! { match true { true => { loop {} }, false => 1 } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { match true { true => 1, false => return \"unreachable\" } } fn main() { let value: int = inferred() }"
        )
        .is_ok());
        assert!(check(
            "fn read() -> int { match true { true if false => \"unreachable\", true => 1, false => 0 } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn read() -> int { match true { true => 1, false => true + 1 } } fn main() {}"
        )
        .is_err());
    }

    #[test]
    fn known_match_values_track_only_executable_arms() {
        assert!(check(
            "fn read() -> int { match 1 { 1 => 42, _ => \"unreachable\" } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn read() -> string { match \"Titan\" { \"Titan\" => \"ok\", _ => 0 } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn inferred() { match 1 { 1 => 42, _ => return \"unreachable\" } } fn main() { let value: int = inferred() }"
        )
        .is_ok());
        assert!(check(
            "enum Choice { First, Second } fn read() -> int { match (Choice::First) { Choice::First => 1, Choice::Second => \"unreachable\" } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "enum Choice { Number(int), Empty } fn read() -> int { match Choice::Number(7) { Choice::Number(value) => value, Choice::Empty => \"unreachable\" } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "enum Choice { First, Second } fn read() -> int { match (Choice::First) { Choice::First if false => \"unreachable\", Choice::First => 1, Choice::Second => 0 } } fn main() {}"
        )
        .is_ok());
        assert!(check(
            "fn read() -> int { match 1 { 1 => 42, _ => true + 1 } } fn main() {}"
        )
        .is_err());
    }

    #[test]
    fn open_ended_matches_require_a_catch_all() {
        assert!(
            check("fn read(value: int) -> int { match value { 1 => 1 } } fn main() {}").is_err()
        );
        assert!(check(
            "fn read(value: int) -> int { match value { 1 => 1, _ => 0 } } fn main() {}"
        )
        .is_ok());
    }

    #[test]
    fn guarded_patterns_are_not_considered_exhaustive() {
        let source = "fn read(value: bool) -> int { match value { true if value => 1, false => 0 } } fn main() {}";
        assert!(check(source).is_err());

        let fallback = "fn read(value: bool) -> int { match value { true if value => 1, true => 2, false => 0 } } fn main() {}";
        assert!(check(fallback).is_ok());
    }

    #[test]
    fn rejects_unreachable_match_patterns() {
        let duplicate_bool = "fn read(value: bool) -> int { match value { true => 1, true => 2, false => 0 } } fn main() {}";
        let errors = check(duplicate_bool).unwrap_err();
        assert!(errors.contains(&TypeError::UnreachablePattern { arm: 2 }));

        let after_catchall =
            "fn read(value: int) -> int { match value { _ => 0, 1 => 1 } } fn main() {}";
        let errors = check(after_catchall).unwrap_err();
        assert!(errors.contains(&TypeError::UnreachablePattern { arm: 2 }));

        let duplicate_variant = "enum Choice { First, Second } fn read(value: Choice) -> int { match value { Choice::First => 1, Choice::First => 2, Choice::Second => 0 } } fn main() {}";
        let errors = check(duplicate_variant).unwrap_err();
        assert!(errors.contains(&TypeError::UnreachablePattern { arm: 2 }));

        let after_finite_coverage = "fn read(value: bool) -> int { match value { true => 1, false => 0, _ => 2 } } fn main() {}";
        let errors = check(after_finite_coverage).unwrap_err();
        assert!(errors.contains(&TypeError::UnreachablePattern { arm: 3 }));

        let guarded_after_coverage = "fn read(value: bool) -> int { match value { true => 1, true if value => 2, false => 0 } } fn main() {}";
        let errors = check(guarded_after_coverage).unwrap_err();
        assert!(errors.contains(&TypeError::UnreachablePattern { arm: 2 }));
    }

    #[test]
    fn dynamic_matches_require_a_real_catchall() {
        let enum_only = "fn read(value: any) -> int { match value { Option::Some(inner) => 1, Option::None => 0 } } fn main() {}";
        assert!(check(enum_only).is_err());

        let with_catchall = "fn read(value: any) -> int { match value { Option::Some(inner) => 1, Option::None => 0, _ => -1 } } fn main() {}";
        assert!(check(with_catchall).is_ok());
    }

    #[test]
    fn exhaustive_finite_matches_count_as_control_flow_exits() {
        let source = "fn stop(value: bool) -> int { let never = match value { true => return 1, false => return 2 } } fn main() {}";
        assert!(check(source).is_ok());
    }

    #[test]
    fn or_patterns_must_bind_the_same_names() {
        let source = "enum Choice { Number(int), Empty } fn read(choice: Choice) -> int { match choice { Choice::Number(value) | Choice::Empty => value, _ => 0 } } fn main() {}";
        assert!(check(source).is_err());
    }
}
