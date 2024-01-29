use anyhow::{Error, Result};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use axum_server::tls_rustls::RustlsConfig;
use figment::{Figment, error, providers::{Env, Format, Yaml}};
use figment::providers::Serialized;
use serde::{Deserialize, Serialize};
use crate::error::ConfigError;

static ENV_PREFIX: &'static str = "PD_";
static ENV_CONFIG_FILE: &'static str = "PD_CONFIG_FILE";
static DEFAULT_CONFIG_FILE: &'static str = "pd-config.yaml";

impl Config {
	pub fn load() -> error::Result<Self> {
		let config_file = std::env::var(ENV_CONFIG_FILE).unwrap_or(DEFAULT_CONFIG_FILE.into());

		Figment::from(Serialized::defaults(Config::default()))
			.merge(Yaml::file(config_file))
			.merge(Env::prefixed(ENV_PREFIX).split("_"))
			.extract()
	}
}

#[derive(Deserialize, Serialize, Default, Debug, PartialEq)]
pub enum Conflict {
	Ignore,
	Override,
	#[default]
	Reject
}

#[derive(Deserialize, Serialize, Default, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Config {
	pub groups: HashMap<String, GroupConfig>,
	pub server: ServerConfig,
}

#[derive(Deserialize, Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
	bind_addr: IpAddr,
	port: u16,
	pub insecure: bool,
	pub cert: PathBuf,
	pub key: PathBuf,
}

impl ServerConfig {
	pub fn socker_addr(&self) -> SocketAddr {
		SocketAddr::new(self.bind_addr, self.port)
	}

	pub async fn tls_config(&self) -> Result<RustlsConfig> {
		match RustlsConfig::from_pem_file(&self.cert, &self.key).await {
			Ok(v) => Ok(v),
			Err(e) => {
				Err(Error::from(ConfigError::TlsConfig {
					source: Error::from(e),
					cert_path: self.cert.clone(),
					key_path: self.key.clone(),
				}))
			}
		}
	}
}

impl Default for ServerConfig {
	fn default() -> Self {
		ServerConfig {
			bind_addr: IpAddr::from([0, 0, 0, 0]),
			port: 8443,
			insecure: false,
			cert: PathBuf::from("cert.pem"),
			key: PathBuf::from("key.pem"),
		}
	}
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupConfig {
	pub node_selector: Option<HashMap<String, String>>,
	pub affinity: Option<Vec<String>>,
	pub tolerations: Option<Vec<String>>,
	pub on_conflict: Conflict,
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;
	use figment::Jail;
	use indoc::indoc;
	use super::{Config, DEFAULT_CONFIG_FILE, ENV_CONFIG_FILE, GroupConfig};

	#[test]
	fn given_valid_config_file_at_default_path_then_should_be_loaded() {
		Jail::expect_with(|jail| {
			jail.create_file(DEFAULT_CONFIG_FILE, indoc! { r#"
				groups:
				  foo:
				    nodeSelector:
					  a: "1"
					  b: "2"
					  c: "3"
				  bar:
				    tolerations: ["1", "2"]
				  bazz:
				    affinity: []
				  all:
				    nodeSelector: {"a": "1", "b": "2", "c": "3"}
				    tolerations: ["1", "2"]
				    affinity: []
			"# })?;

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("foo".into(), GroupConfig {
				node_selector: Some(HashMap::from([
					("a".into(), "1".into()),
					("b".into(), "2".into()),
					("c".into(), "3".into()),
				])),
				affinity: None,
				tolerations: None,
				on_conflict: Default::default(),
			});
			groups.insert("bar".into(), GroupConfig {
				node_selector: None,
				affinity: None,
				tolerations: Some(vec!["1".into(), "2".into()]),
				on_conflict: Default::default(),
			});
			groups.insert("bazz".into(), GroupConfig {
				node_selector: None,
				affinity: Some(vec![]),
				tolerations: None,
				on_conflict: Default::default(),
			});
			groups.insert("all".into(), GroupConfig {
				node_selector: Some(HashMap::from([
					("a".into(), "1".into()),
					("b".into(), "2".into()),
					("c".into(), "3".into()),
				])),
				affinity: Some(vec![]),
				tolerations: Some(vec!["1".into(), "2".into()]),
				on_conflict: Default::default(),
			});

			assert_eq!(config, Config { groups, server: Default::default() });

			Ok(())
		});
	}

	#[test]
	fn given_different_file_path_by_env_var_should_load_correct_file() {
		Jail::expect_with(|jail| {
			jail.create_file(DEFAULT_CONFIG_FILE, indoc! { r#"
				groups:
				  foo:
				    nodeSelector: {}
			"# })?;

			jail.create_file("other-config-file.yaml", indoc! { r#"
				groups:
				  bar:
				    nodeSelector: {"a": "1"}
			"# })?;

			jail.set_env(ENV_CONFIG_FILE, "other-config-file.yaml");

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("bar".into(), GroupConfig {
				node_selector: Some(HashMap::from([("a".into(), "1".into())])),
				affinity: None,
				tolerations: None,
				on_conflict: Default::default(),
			});

			assert_eq!(config, Config { groups, server: Default::default() });

			Ok(())
		});
	}

	#[test]
	fn given_value_provided_by_env_then_should_load_value() {
		Jail::expect_with(|jail| {
			jail.create_file(DEFAULT_CONFIG_FILE, indoc! { r#"
				groups:
				  foo:
				    affinity: ["x", "y"]
			"# })?;
			jail.set_env("PD_GROUPS_FOO_AFFINITY", r#"["a", "b"]"#);

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("foo".into(), GroupConfig {
				node_selector: None,
				affinity: Some(vec!["a".into(), "b".into()]),
				tolerations: None,
				on_conflict: Default::default(),
			});

			assert_eq!(config, Config { groups, server: Default::default() });

			Ok(())
		});
	}

	#[test]
	fn given_value_provided_by_env_and_by_file_then_should_load_value_from_env() {
		Jail::expect_with(|jail| {
			jail.set_env("PD_GROUPS_FOO_AFFINITY", r#"["a", "b"]"#);

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("foo".into(), GroupConfig {
				node_selector: None,
				affinity: Some(vec!["a".into(), "b".into()]),
				tolerations: None,
				on_conflict: Default::default(),
			});

			assert_eq!(config, Config { groups, server: Default::default() });

			Ok(())
		});
	}
}
