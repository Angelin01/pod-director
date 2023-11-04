use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
	groups: HashMap<String, GroupConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupConfig {
	node_selector: Vec<String>,
	affinity: Vec<String>,
	tolerations: Vec<String>,
}
