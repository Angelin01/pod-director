use std::collections::HashMap;
use figment::{Figment, error, providers::{Env, Format, Yaml}};
use serde::Deserialize;

static ENV_PREFIX: &'static str = "PD_";
static ENV_CONFIG_FILE: &'static str = "PD_CONFIG_FILE";
static DEFAULT_CONFIG_FILE: &'static str = "pd-config.yaml";

impl Config {
	pub fn load() -> error::Result<Self> {
		let config_file = std::env::var(ENV_CONFIG_FILE).unwrap_or(DEFAULT_CONFIG_FILE.into());

		Figment::new()
			.merge(Yaml::file(config_file))
			.merge(Env::prefixed(ENV_PREFIX).split("__"))
			.extract()
	}
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct Config {
	groups: HashMap<String, GroupConfig>,
}

#[derive(Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct GroupConfig {
	node_selector: Option<Vec<String>>,
	affinity: Option<Vec<String>>,
	tolerations: Option<Vec<String>>,
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
				    nodeSelector: ["a", "b", "c"]
				  bar:
				    tolerations: ["1", "2"]
				  bazz:
				    affinity: []
				  all:
				    nodeSelector: ["a", "b", "c"]
				    tolerations: ["1", "2"]
				    affinity: []
			"# })?;

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("foo".into(), GroupConfig {
				node_selector: Some(vec!["a".into(), "b".into(), "c".into()]),
				affinity: None,
				tolerations: None,
			});
			groups.insert("bar".into(), GroupConfig {
				node_selector: None,
				affinity: None,
				tolerations: Some(vec!["1".into(), "2".into()]),
			});
			groups.insert("bazz".into(), GroupConfig {
				node_selector: None,
				affinity: Some(vec![]),
				tolerations: None,
			});
			groups.insert("all".into(), GroupConfig {
				node_selector: Some(vec!["a".into(), "b".into(), "c".into()]),
				affinity: Some(vec![]),
				tolerations: Some(vec!["1".into(), "2".into()]),
			});

			assert_eq!(config, Config { groups });

			Ok(())
		});
	}

	#[test]
	fn given_different_file_path_by_env_var_should_load_correct_file() {
		Jail::expect_with(|jail| {
			jail.create_file(DEFAULT_CONFIG_FILE, indoc! { r#"
				groups:
				  foo:
				    nodeSelector: []
			"# })?;

			jail.create_file("other-config-file.yaml", indoc! { r#"
				groups:
				  bar:
				    nodeSelector: ["a"]
			"# })?;

			jail.set_env(ENV_CONFIG_FILE, "other-config-file.yaml");

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("bar".into(), GroupConfig {
				node_selector: Some(vec!["a".into()]),
				affinity: None,
				tolerations: None,
			});

			assert_eq!(config, Config { groups });

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
			jail.set_env("PD_GROUPS__FOO__AFFINITY", r#"["a", "b"]"#);

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("foo".into(), GroupConfig {
				node_selector: None,
				affinity: Some(vec!["a".into(), "b".into()]),
				tolerations: None,
			});

			assert_eq!(config, Config { groups });

			Ok(())
		});
	}

	#[test]
	fn given_value_provided_by_env_and_by_file_then_should_load_value_from_env() {
		Jail::expect_with(|jail| {
			jail.set_env("PD_GROUPS__FOO__AFFINITY", r#"["a", "b"]"#);

			let config = Config::load()?;

			let mut groups = HashMap::new();
			groups.insert("foo".into(), GroupConfig {
				node_selector: None,
				affinity: Some(vec!["a".into(), "b".into()]),
				tolerations: None,
			});

			assert_eq!(config, Config { groups });

			Ok(())
		});
	}
}
