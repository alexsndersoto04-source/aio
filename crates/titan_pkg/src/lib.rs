//! Titan Package Manager
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Error, Debug)]
pub enum PkgError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, PkgError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub package: PackageInfo,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub edition: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub version: Option<String>,
    pub path: Option<String>,
}

impl Manifest {
    pub fn from_dir(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path.join("Titan.toml"))
            .map_err(|_| PkgError::NotFound("Titan.toml".into()))?;
        toml::from_str(&content).map_err(|e| PkgError::NotFound(e.to_string()))
    }
    pub fn new(name: &str) -> Self {
        Manifest {
            package: PackageInfo {
                name: name.into(), version: "0.1.0".into(), edition: "2021".into(),
                description: String::new(), license: "MIT".into(),
            },
            dependencies: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub source: String,
}