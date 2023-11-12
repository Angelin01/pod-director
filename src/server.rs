mod health_handler;

use anyhow::Result;
use axum::Router;
use axum::routing::get;
use health_handler::health_handler;
use crate::config::Config;

fn build_app() -> Router {
	Router::new()
		.route("/health", get(health_handler))
}

pub async fn serve(config: &Config) -> Result<()> {
	let addr = config.server.socker_addr();
	Ok(match config.server.insecure {
		true => {
			axum_server::bind(addr)
				.serve(build_app().into_make_service())
				.await?;
		}
		false => {
			let tls_config = config.server.tls_config().await?;
			axum_server::bind_rustls(addr, tls_config)
				.serve(build_app().into_make_service())
				.await?;
		}
	})
}
