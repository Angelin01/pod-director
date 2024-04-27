use std::sync::Arc;
use crate::config::Config;
use crate::service::{KubernetesService, StandardKubernetesService};

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

#[cfg(test)]
pub mod tests {
	use std::sync::Arc;
	use crate::config::Config;
	use crate::server::AppState;
	use crate::service::tests::MockKubernetesService;

	#[derive(Clone)]
	pub struct TestAppState {
		config: Arc<Config>,
		pub kubernetes: MockKubernetesService,
	}

	impl TestAppState {
		pub fn new(config: Config) -> Self {
			Self {
				config: Arc::new(config),
				kubernetes: MockKubernetesService::new()
			}
		}
	}

	impl AppState for TestAppState {
		type K = MockKubernetesService;

		fn config(&self) -> &Config {
			&self.config
		}

		fn kubernetes(&self) -> &Self::K {
			&self.kubernetes
		}
	}
}
