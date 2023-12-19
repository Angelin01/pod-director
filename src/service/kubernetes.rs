use anyhow::Result;
use k8s_openapi::api::core::v1::Namespace;
use kube::{Api, Client, ResourceExt};

const LABEL: &str = "pod-director/group";

// TODO: receive client as argument, or make it part of a struct that is shared as state
// TODO: cache this info so we don't call the api server every time, make it configurable
pub async fn namespace_group<S: AsRef<str>>(namespace: S) -> Result<Option<String>> {
	let client = Client::try_default().await?;
	let api: Api<Namespace> = Api::all(client);

	let namespace = api.get(namespace.as_ref()).await?;

	let result = match namespace.labels().get(LABEL) {
		None => None,
		Some(s) => Some(s.to_string())
	};

	Ok(result)
}
