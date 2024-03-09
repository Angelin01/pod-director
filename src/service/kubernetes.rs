use anyhow::Result;
use axum::async_trait;
use k8s_openapi::api::core::v1::Namespace;
use kube::{Api, Client, ResourceExt};

const LABEL: &str = "pod-director/group";

#[async_trait]
pub trait KubernetesService: Send + Sync + Clone {
	async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace: S) -> Result<Option<String>>;
}

#[derive(Clone)]
pub struct StandardKubernetesService {
	api: Api<Namespace>,
}

impl StandardKubernetesService {
	pub async fn new() -> Result<Self> {
		Ok(StandardKubernetesService {
			api: Api::all(Client::try_default().await?),
		})
	}
}

// TODO: Use a watcher or reconciller to watch events and avoid callin the API every time
#[async_trait]
impl KubernetesService for StandardKubernetesService {
	async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace: S) -> Result<Option<String>> {
		let namespace = self.api.get(namespace.as_ref()).await?;

		let result = match namespace.labels().get(LABEL) {
			None => None,
			Some(s) => Some(s.to_string())
		};

		Ok(result)
	}
}
