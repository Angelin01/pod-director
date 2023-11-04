use std::collections::HashMap;
use figment::{Figment, providers::{Env, Format, Yaml}};
use serde::Deserialize;

static ENV_PREFIX: &'static str = "PD_";
static ENV_CONFIG_FILE: &'static str = "PD_CONFIG_FILE";
static DEFAULT_CONFIG_FILE: &'static str = "./pd-config.yaml";

impl Config {
	pub fn load() -> Self {
		let config_file = std::env::var(ENV_CONFIG_FILE).unwrap_or(DEFAULT_CONFIG_FILE.into());

		Figment::new()
			.merge(Yaml::file(config_file))
			.merge(Env::prefixed(ENV_PREFIX).split("__"))
			.extract()
			.unwrap()
	}
}

#[derive(Deserialize, Debug)]
pub struct Config {
	groups: HashMap<String, GroupConfig>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GroupConfig {
	node_selector: Option<Vec<String>>,
	affinity: Option<Vec<String>>,
	tolerations: Option<Vec<String>>,
}
