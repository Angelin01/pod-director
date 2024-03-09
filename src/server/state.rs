use std::sync::Arc;
use crate::config::Config;
use crate::service::kubernetes::{KubernetesService, StandardKubernetesService};

pub trait AppState: Clone + Send + Sync + 'static {
	type K: KubernetesService;

	fn config(&self) -> &Config;
	fn kubernetes(&self) -> &Self::K;
}

#[derive(Clone)]
pub struct StandardAppState {
	config: Arc<Config>,
	kubernetes: StandardKubernetesService,
}

impl StandardAppState {
	pub fn new(config: Arc<Config>, kubernetes: StandardKubernetesService) -> Self {
		Self { config, kubernetes }
	}
}

impl AppState for StandardAppState {
	type K = StandardKubernetesService;

	fn config(&self) -> &Config {
		&self.config
	}

	fn kubernetes(&self) -> &Self::K {
		&self.kubernetes
	}
}
