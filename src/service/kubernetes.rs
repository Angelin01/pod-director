use axum::async_trait;
use k8s_openapi::api::core::v1::Namespace;
use kube::{Api, Client, ResourceExt};

const LABEL: &str = "pod-director/group";

#[async_trait]
pub trait KubernetesService: Send + Sync + Clone {
	async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace: S) -> Result<Option<String>, kube::error::Error>;
}

#[derive(Clone)]
pub struct StandardKubernetesService {
	api: Api<Namespace>,
}

impl StandardKubernetesService {
	pub async fn new() -> anyhow::Result<Self> {
		Ok(StandardKubernetesService {
			api: Api::all(Client::try_default().await?),
		})
	}
}

// TODO: Use a watcher or reconciller to watch events and avoid callin the API every time
#[async_trait]
impl KubernetesService for StandardKubernetesService {
	async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace: S) -> Result<Option<String>, kube::error::Error> {
		let namespace = self.api.get(namespace.as_ref()).await?;

		let result = match namespace.labels().get(LABEL) {
			None => None,
			Some(s) => Some(s.to_string())
		};

		Ok(result)
	}
}

#[cfg(test)]
pub mod tests {
	use std::collections::BTreeMap;
	use axum::async_trait;
	use kube::error::ErrorResponse;
	use crate::service::kubernetes::KubernetesService;

	#[derive(Clone)]
	pub struct MockKubernetesService {
		namespace_group_map: BTreeMap<String, String>,
		error: bool,
	}

	impl MockKubernetesService {
		pub fn new() -> Self {
			MockKubernetesService {
				namespace_group_map: BTreeMap::new(),
				error: false,
			}
		}
		pub fn set_namespace_group<S: AsRef<str>, R: AsRef<str>>(&mut self, namespace: S, group: R) {
			self.namespace_group_map.insert(namespace.as_ref().into(), group.as_ref().into());
		}

		pub fn set_error(&mut self) {
			self.error = true;
		}
	}

	#[async_trait]
	impl KubernetesService for MockKubernetesService {
		async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace: S) -> Result<Option<String>, kube::error::Error> {
			if self.error {
				return Err(kube::error::Error::Api(ErrorResponse {
					code: 400,
					message: "Some Kubernetes error".into(),
					reason: "Some reason".into(),
					status: "Failed".into(),
				}));
			}

			Ok(self.namespace_group_map.get(namespace.as_ref()).map(String::to_owned))
		}
	}
}
