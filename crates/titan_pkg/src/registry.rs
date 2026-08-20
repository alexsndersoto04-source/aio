//! HTTPS package registry resolution, integrity verification, and cache.
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("registry URL must use HTTPS")]
    HttpsRequired,
    #[error("invalid registry package name")]
    InvalidName,
    #[error("registry package name mismatch")]
    PackageNameMismatch,
    #[error("registry request failed: {0}")]
    Http(String),
    #[error("registry returned HTTP {0}")]
    Status(u16),
    #[error("invalid registry metadata: {0}")]
    Metadata(#[from] serde_json::Error),
    #[error("package '{0}' has no matching version")]
    NoVersion(String),
    #[error("invalid semantic version/range: {0}")]
    Semver(String),
    #[error("package SHA-256 mismatch")]
    Checksum,
    #[error("package Ed25519 signature is invalid")]
    Signature,
    #[error("package exceeds configured download limit")]
    TooLarge,
    #[error("package '{package}' is not in the offline cache")]
    OfflineCacheMiss { package: String },
    #[error("cache I/O error: {0}")]
    Io(#[from] std::io::Error),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageIndex {
    pub name: String,
    pub versions: Vec<PackageVersion>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersion {
    pub version: String,
    pub archive: String,
    pub sha256: String,
    pub signing_key: String,
    pub signature: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}
#[derive(Debug, Clone)]
pub struct RegistryClient {
    base: String,
    cache: PathBuf,
    timeout: Duration,
    max_archive: usize,
}
impl RegistryClient {
    pub fn new(base: &str, cache: impl Into<PathBuf>) -> Result<Self, RegistryError> {
        if !base.starts_with("https://") {
            return Err(RegistryError::HttpsRequired);
        }
        Ok(Self {
            base: base.trim_end_matches('/').into(),
            cache: cache.into(),
            timeout: Duration::from_secs(30),
            max_archive: 64 * 1024 * 1024,
        })
    }
    pub fn fetch_index(&self, name: &str) -> Result<PackageIndex, RegistryError> {
        validate_name(name)?;
        let encoded = titan_stdlib::encoding::percent_encode(name);
        let response = self.get(&format!("{}/v1/packages/{encoded}", self.base), 1024 * 1024)?;
        let index: PackageIndex = serde_json::from_slice(&response)?;
        if index.name != name {
            return Err(RegistryError::PackageNameMismatch);
        }
        Ok(index)
    }
    pub fn resolve<'a>(
        &self,
        index: &'a PackageIndex,
        requirement: &str,
    ) -> Result<&'a PackageVersion, RegistryError> {
        let requirement = semver::VersionReq::parse(requirement)
            .map_err(|error| RegistryError::Semver(error.to_string()))?;
        index
            .versions
            .iter()
            .filter_map(|release| {
                semver::Version::parse(&release.version)
                    .ok()
                    .map(|version| (version, release))
            })
            .filter(|(version, _)| requirement.matches(version))
            .max_by(|(left, _), (right, _)| left.cmp(right))
            .map(|(_, release)| release)
            .ok_or_else(|| RegistryError::NoVersion(index.name.clone()))
    }
    pub fn download(
        &self,
        name: &str,
        release: &PackageVersion,
        allow_network: bool,
    ) -> Result<PathBuf, RegistryError> {
        validate_name(name)?;
        semver::Version::parse(&release.version)
            .map_err(|error| RegistryError::Semver(error.to_string()))?;
        validate_hash(&release.sha256)?;
        verify_signature(release)?;
        let destination = self
            .cache
            .join(name)
            .join(&release.version)
            .join(format!("{}.tpkg", release.sha256));
        if destination.is_file() {
            let bytes = std::fs::read(&destination)?;
            verify(&bytes, &release.sha256)?;
            return Ok(destination);
        }
        // Offline mode (allow_network == false) must never fall back to the
        // network: a cache miss is an explicit error so `titan fetch
        // --offline` cannot leak requests for uncached packages.
        if !allow_network {
            return Err(RegistryError::OfflineCacheMiss {
                package: name.to_string(),
            });
        }
        let bytes = self.get(&release.archive, self.max_archive)?;
        verify(&bytes, &release.sha256)?;
        let parent = destination.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let temporary = destination.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&temporary, &bytes)?;
        match std::fs::rename(&temporary, &destination) {
            Ok(()) => {}
            Err(_error) if destination.is_file() => {
                let _ = std::fs::remove_file(&temporary);
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(error.into());
            }
        }
        Ok(destination)
    }
    fn get(&self, url: &str, maximum: usize) -> Result<Vec<u8>, RegistryError> {
        if !url.starts_with("https://") {
            return Err(RegistryError::HttpsRequired);
        }
        let response = titan_stdlib::http_client::request(titan_stdlib::http_client::Request {
            method: "GET".into(),
            url: url.into(),
            headers: BTreeMap::from([(
                "Accept".into(),
                "application/json, application/octet-stream".into(),
            )]),
            body: Vec::new(),
            maximum_body: maximum,
            redirects: 3,
            timeout: self.timeout,
        })
        .map_err(|error| RegistryError::Http(error.to_string()))?;
        if !response.final_url.starts_with("https://") {
            return Err(RegistryError::HttpsRequired);
        }
        if response.status != 200 {
            return Err(RegistryError::Status(response.status));
        }
        Ok(response.body)
    }
}
fn verify_signature(release: &PackageVersion) -> Result<(), RegistryError> {
    let key = titan_stdlib::encoding::base64_decode(&release.signing_key)
        .map_err(|_| RegistryError::Signature)?;
    let signature = titan_stdlib::encoding::base64_decode(&release.signature)
        .map_err(|_| RegistryError::Signature)?;
    let key: [u8; 32] = key.try_into().map_err(|_| RegistryError::Signature)?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| RegistryError::Signature)?;
    let signature = Signature::from_slice(&signature).map_err(|_| RegistryError::Signature)?;
    let digest = hex_digest(&release.sha256)?;
    key.verify(&digest, &signature)
        .map_err(|_| RegistryError::Signature)
}
fn hex_digest(value: &str) -> Result<[u8; 32], RegistryError> {
    validate_hash(value)?;
    let bytes = titan_stdlib::encoding::hex_decode(value).map_err(|_| RegistryError::Checksum)?;
    bytes.try_into().map_err(|_| RegistryError::Checksum)
}
fn validate_name(name: &str) -> Result<(), RegistryError> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Err(RegistryError::InvalidName)
    } else {
        Ok(())
    }
}
fn validate_hash(expected: &str) -> Result<(), RegistryError> {
    if expected.len() != 64 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Err(RegistryError::Checksum)
    } else {
        Ok(())
    }
}
fn verify(bytes: &[u8], expected: &str) -> Result<(), RegistryError> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(RegistryError::Checksum)
    }
}
pub fn cached_package(cache: &Path, name: &str, version: &str, sha256: &str) -> PathBuf {
    cache
        .join(name)
        .join(version)
        .join(format!("{sha256}.tpkg"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_highest_matching_semver() {
        let client = RegistryClient::new("https://registry.example", std::env::temp_dir()).unwrap();
        let index = PackageIndex {
            name: "demo".into(),
            versions: vec!["1.0.0", "1.5.0", "2.0.0"]
                .into_iter()
                .map(|version| PackageVersion {
                    version: version.into(),
                    archive: "https://registry.example/a".into(),
                    sha256: "0".repeat(64),
                    signing_key: "A".repeat(44),
                    signature: "A".repeat(88),
                    dependencies: BTreeMap::new(),
                })
                .collect(),
        };
        assert_eq!(client.resolve(&index, "^1.0").unwrap().version, "1.5.0");
        assert!(client.resolve(&index, "^3").is_err());
    }
    #[test]
    fn verifies_sha256_and_rejects_insecure_registry() {
        assert!(RegistryClient::new("http://registry.example", "cache").is_err());
        assert!(validate_name("../escape").is_err());
        let hash = format!("{:x}", Sha256::digest(b"titan"));
        assert!(verify(b"titan", &hash).is_ok());
        assert!(verify(b"changed", &hash).is_err());
    }
    #[test]
    fn offline_download_never_hits_the_network_on_cache_miss() {
        use ed25519_dalek::{Signer, SigningKey};
        let cache = std::env::temp_dir().join(format!("titan-offline-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        let client = RegistryClient::new("https://registry.example", cache.clone()).unwrap();
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let digest = Sha256::digest(b"archive");
        let signature = signing.sign(&digest);
        let release = PackageVersion {
            version: "1.0.0".into(),
            archive: "https://registry.example/pkg.tpkg".into(),
            sha256: format!("{:x}", digest),
            signing_key: titan_stdlib::encoding::base64_encode(&signing.verifying_key().to_bytes()),
            signature: titan_stdlib::encoding::base64_encode(&signature.to_bytes()),
            dependencies: BTreeMap::new(),
        };
        // Cache is empty and network is disallowed (offline): must fail with
        // OfflineCacheMiss WITHOUT attempting any network request.
        let result = client.download("pkg", &release, false);
        let _ = std::fs::remove_dir_all(&cache);
        assert!(
            matches!(result, Err(RegistryError::OfflineCacheMiss { package }) if package == "pkg"),
            "offline download must not hit the network: {result:?}"
        );
    }
    #[test]
    fn verifies_ed25519_release_signature() {
        use ed25519_dalek::{Signer, SigningKey};
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let digest = Sha256::digest(b"archive");
        let signature = signing.sign(&digest);
        let release = PackageVersion {
            version: "1.0.0".into(),
            archive: "https://example/a".into(),
            sha256: format!("{digest:x}"),
            signing_key: titan_stdlib::encoding::base64_encode(&signing.verifying_key().to_bytes()),
            signature: titan_stdlib::encoding::base64_encode(&signature.to_bytes()),
            dependencies: BTreeMap::new(),
        };
        assert!(verify_signature(&release).is_ok());
    }
}
