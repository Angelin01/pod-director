use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
	#[error("failed loading certificates (cert: \"{cert_path}\"; and key: \"{key_path}\"): {source}")]
	TlsConfig { source: anyhow::Error, cert_path: PathBuf, key_path: PathBuf }
}
