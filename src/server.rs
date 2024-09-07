use std::sync::Arc;
use anyhow::{Error, Result};
use axum::Router;
use axum::routing::{get, post};
use axum_server::Handle;
use crate::config::Config;
use super::handler;

pub use state::{AppState, StandardAppState};

use crate::service::StandardKubernetesService;

mod tls;
mod shutdown;
pub mod state;

pub fn build_app<S: AppState>(state: S) -> Router {
	Router::new()
		.route("/health", get(handler::health::<S>))
		.route("/mutate", post(handler::mutate::<S>))
		.with_state(state)
}

pub async fn serve(config: Arc<Config>) -> Result<()> {
	let addr = config.server.socker_addr();

	let shutdown_handle = Handle::new();
	tokio::spawn(shutdown::graceful_shutdown(shutdown_handle.clone()));

	let kubernetes = StandardKubernetesService::new(&config.group_label).await?;
	let app_state = StandardAppState::new(config.clone(), kubernetes);
	let service = build_app(app_state).into_make_service();

	println!("Server starting, listening on {addr}");

	if config.server.insecure {
		axum_server::bind(addr)
			.handle(shutdown_handle)
			.serve(service)
			.await?;
	}
	else {
		let tls_config = config.server.tls_config().await?;
		let hot_reload = tokio::spawn(tls::hot_reload_tls(
			tls_config.clone(),
			config.server.cert.clone(),
			config.server.key.clone(),
		));

		let result = axum_server::bind_rustls(addr, tls_config)
			.handle(shutdown_handle)
			.serve(service)
			.await;

		hot_reload.abort();

		if let Err(e) = result {
			return Err(Error::new(e));
		}
	}

	Ok(())
}
