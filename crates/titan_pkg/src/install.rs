//! Project-level remote dependency synchronization.
use crate::{
    archive::{self, ExtractionLimits},
    resolve_remote, Manifest, PackageVersion, RegistryClient, RemoteLockfile,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
#[derive(Error, Debug)]
pub enum InstallError {
    #[error("invalid dependency: {0}")]
    Dependency(String),
    #[error("package error: {0}")]
    Package(#[from] crate::PkgError),
    #[error("registry error: {0}")]
    Registry(#[from] crate::RegistryError),
    #[error("resolution error: {0}")]
    Resolve(#[from] crate::ResolveError),
    #[error("archive error: {0}")]
    Archive(#[from] crate::archive::ArchiveError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("remote dependency '{0}' has no version requirement")]
    MissingVersion(String),
    #[error("offline lockfile is missing")]
    OfflineLock,
}
pub fn sync(
    root: &Path,
    registry_url: &str,
    offline: bool,
) -> Result<RemoteLockfile, InstallError> {
    let manifest = Manifest::from_dir(root)?;
    let roots: BTreeMap<String, String> = manifest
        .dependencies
        .iter()
        .filter(|(_, dependency)| dependency.path.is_none())
        .map(|(name, dependency)| {
            dependency
                .version
                .clone()
                .map(|version| (name.clone(), version))
                .ok_or_else(|| InstallError::MissingVersion(name.clone()))
        })
        .collect::<Result<_, _>>()?;
    let titan = root.join(".titan");
    let lock_path = root.join("Titan.remote.lock");
    let client = RegistryClient::new(registry_url, titan.join("cache"))?;
    let lock = if offline {
        if !lock_path.is_file() {
            return Err(InstallError::OfflineLock);
        }
        RemoteLockfile::read(&lock_path)?
    } else {
        resolve_remote(&client, &roots, 10_000, 100_000)?
    };
    for package in &lock.packages {
        let release = PackageVersion {
            version: package.version.clone(),
            archive: package.archive.clone(),
            sha256: package.sha256.clone(),
            signing_key: package.signing_key.clone(),
            signature: package.signature.clone(),
            dependencies: package.dependencies.clone(),
        };
        let archive_path = client.download(&package.name, &release, !offline)?;
        let destination = titan
            .join("packages")
            .join(&package.name)
            .join(&package.version);
        if !destination.exists() {
            archive::extract(&archive_path, &destination, &ExtractionLimits::default())?;
        }
    }
    if !offline {
        lock.write(&lock_path)?;
    }
    Ok(lock)
}
pub fn add_remote(root: &Path, name: &str, requirement: &str) -> Result<(), InstallError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(InstallError::Dependency("invalid package name".into()));
    }
    semver::VersionReq::parse(requirement)
        .map_err(|error| InstallError::Dependency(error.to_string()))?;
    let mut manifest = Manifest::from_dir(root)?;
    manifest.dependencies.insert(
        name.into(),
        crate::Dependency {
            version: Some(requirement.into()),
            path: None,
        },
    );
    let text = toml::to_string_pretty(&manifest)
        .map_err(|error| InstallError::Dependency(error.to_string()))?;
    let path = root.join("Titan.toml");
    let temporary = root.join("Titan.toml.tmp");
    std::fs::write(&temporary, text)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}
pub fn installed_path(root: &Path, name: &str, version: &str) -> PathBuf {
    root.join(".titan")
        .join("packages")
        .join(name)
        .join(version)
}
