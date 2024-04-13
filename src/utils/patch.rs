use std::collections::HashMap;

use json_patch::PatchOperation;
use k8s_openapi::api::core::v1::Pod;
use serde_json::{json, Value};

use crate::config::Conflict;

pub fn add(path: String, value: Value) -> PatchOperation {
	PatchOperation::Add(json_patch::AddOperation {
		path,
		value,
	})
}

pub fn replace(path: String, value: Value) -> PatchOperation {
	PatchOperation::Replace(json_patch::ReplaceOperation {
		path,
		value,
	})
}

pub enum PatchResult<'a> {
	Allow(Vec<PatchOperation>),
	Deny {
		label: &'a str,
		config_value: &'a str,
		conflicting_value: &'a str,
	},
}

pub fn calculate_node_selector_patches<'a>(
	pod: &'a Pod,
	node_selector_config: &'a HashMap<String, String>,
	conflict_config: &'a Conflict,
) -> PatchResult<'a> {
	let mut patches = Vec::new();

	let maybe_node_selector = pod.spec.as_ref()
		.map_or(None, |spec| spec.node_selector.as_ref());

	if let Some(node_selector) = maybe_node_selector {
		for (k, v) in node_selector_config.iter() {
			match node_selector.get(k) {
				None => patches.push(add(format!("/spec/nodeSelector/{k}"), json!(v))),
				Some(existing_value) if existing_value == v => continue,
				Some(existing_value) => match conflict_config {
					Conflict::Ignore => (),
					Conflict::Override => patches.push(replace(format!("/spec/nodeSelector/{k}"), json!(v))),
					Conflict::Reject => {
						return PatchResult::Deny {
							label: k.as_str(),
							config_value: v.as_str(),
							conflicting_value: existing_value.as_str(),
						};
					}
				},
			}
		}
	} else {
		patches.push(add("/spec/nodeSelector".into(), json!({})));
		node_selector_config.iter().for_each(|(k, v)| {
			patches.push(add(format!("/spec/nodeSelector/{k}"), json!(v)));
		});
	}

	PatchResult::Allow(patches)
}
