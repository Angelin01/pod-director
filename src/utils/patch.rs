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

pub enum PatchResult {
	Allow(Vec<PatchOperation>),
	Deny {
		label: String,
		config_value: String,
		conflicting_value: String,
	},
}

pub fn calculate_node_selector_patches(
	pod: &Pod,
	node_selector_config: &HashMap<String, String>,
	conflict_config: &Conflict,
) -> PatchResult {
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
							label: k.into(),
							config_value: v.into(),
							conflicting_value: existing_value.into(),
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
