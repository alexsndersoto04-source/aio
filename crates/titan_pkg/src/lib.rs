//! Titan package manifests, lockfiles, and multi-file project loading.

pub mod project;
pub use project::{create_project, default_entry, find_project_root, ProjectError, SourceProject};

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
    #[error("invalid Titan.toml: {0}")]
    InvalidManifest(#[from] toml::de::Error),
    #[error("invalid package version '{0}'")]
    InvalidVersion(String),
    #[error("invalid lockfile: {0}")]
    InvalidLockfile(#[from] serde_json::Error),
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
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

impl Manifest {
    pub fn from_dir(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path.join("Titan.toml"))
            .map_err(|_| PkgError::NotFound("Titan.toml".into()))?;
        let manifest: Self = toml::from_str(&content)?;
        semver::Version::parse(&manifest.package.version).map_err(|_| PkgError::InvalidVersion(manifest.package.version.clone()))?;
        Ok(manifest)
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

impl Lockfile {
    pub fn from_dependencies(dependencies: &BTreeMap<String, std::path::PathBuf>) -> Result<Self> {
        let mut packages = Vec::new();
        for (alias, source_root) in dependencies {
            let root = source_root.parent().ok_or_else(|| PkgError::NotFound(source_root.display().to_string()))?;
            let manifest = Manifest::from_dir(root)?;
            packages.push(LockedPackage {
                name: alias.clone(),
                version: manifest.package.version,
                source: format!("path+{}", root.canonicalize()?.display()),
            });
        }
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self { version: 1, packages })
    }

    pub fn read(path: &Path) -> Result<Self> { Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?) }

    pub fn write(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)? + "\n";
        let temporary = path.with_extension("lock.tmp");
        std::fs::write(&temporary, content)?;
        if path.exists() { std::fs::remove_file(path)?; }
        std::fs::rename(temporary, path)?;
        Ok(())
    }
}