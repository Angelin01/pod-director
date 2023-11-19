mod health_handler;

use std::time::Duration;
use anyhow::Result;
use axum::Router;
use axum::routing::get;
use axum_server::Handle;
use tokio::signal;
use health_handler::health_handler;
use crate::config::Config;

fn build_app() -> Router {
	Router::new()
		.route("/health", get(health_handler))
}

pub async fn serve(config: &Config) -> Result<()> {
	let addr = config.server.socker_addr();

	let shutdown_handle = Handle::new();
	tokio::spawn(graceful_shutdown(shutdown_handle.clone()));

	let service = build_app().into_make_service();

	println!("Server starting, listening on {addr}");

	if config.server.insecure {
		axum_server::bind(addr)
			.handle(shutdown_handle)
			.serve(service)
			.await?;
	}
	else {
		let tls_config = config.server.tls_config().await?;
		axum_server::bind_rustls(addr, tls_config)
			.handle(shutdown_handle)
			.serve(service)
			.await?;
	}

	Ok(())
}

async fn graceful_shutdown(handle: Handle) {
	// Wait 10 seconds.
	let ctrl_c = async {
		signal::ctrl_c()
			.await
			.expect("failed to install interrupt handler");
	};

	#[cfg(unix)]
	let terminate = async {
		signal::unix::signal(signal::unix::SignalKind::terminate())
			.expect("failed to install SIGTERM handler")
			.recv()
			.await;
	};

	#[cfg(not(unix))]
	let terminate = std::future::pending::<()>();

	let received_shutdown = tokio::select! {
		biased;
		_ = ctrl_c => true,
		_ = terminate => true,
		else => false
	};

	if received_shutdown {
		println!("Received signal, shutting down");
		handle.graceful_shutdown(Some(Duration::from_secs(30)));
	}
}
