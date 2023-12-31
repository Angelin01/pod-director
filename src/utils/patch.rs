use json_patch::PatchOperation;
use serde_json::Value;

pub fn add(path: String, value: Value) -> PatchOperation {
	PatchOperation::Add(json_patch::AddOperation {
		path,
		value,
	})
}
