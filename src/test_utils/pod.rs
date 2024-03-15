use std::collections::BTreeMap;
use serde_json::Value;

struct PodCreateRequestBuilder {
	namespace: Option<String>,
	node_selector: Option<BTreeMap<String, String>>,
}

impl PodCreateRequestBuilder {
	pub fn new() -> Self {
		Self { namespace: None, node_selector: None }
	}

	pub fn with_namespace<S: AsRef<str>>(mut self, namespace: S) -> Self {
		self.namespace = Some(namespace.as_ref().to_string());
		self
	}

	pub fn with_node_selector<S: AsRef<str>, R: AsRef<str>>(mut self, label: S, value: R) -> Self {
		self.node_selector.get_or_insert_with(BTreeMap::new)
			.insert(label.as_ref().into(), value.as_ref().into());
		self
	}

	pub fn build(self) -> Value {

	}
}
