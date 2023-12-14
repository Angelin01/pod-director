mod health_handler;
mod mutate_handler;

use crate::config::Config;
use anyhow::{Error, Result};
use axum::routing::{get, post};
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use health_handler::health_handler;
use mutate_handler::mutate_handler;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebouncedEvent, Debouncer, FileIdMap};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc::Receiver;

fn build_app(config: Arc<Config>) -> Router {
	Router::new()
		.route("/health", get(health_handler))
		.route("/mutate", post(mutate_handler))
		.with_state(config)
}

pub async fn serve(config: Arc<Config>) -> Result<()> {
	let addr = config.server.socker_addr();

	let shutdown_handle = Handle::new();
	tokio::spawn(graceful_shutdown(shutdown_handle.clone()));

	let service = build_app(config.clone()).into_make_service();

	println!("Server starting, listening on {addr}");

	if config.server.insecure {
		axum_server::bind(addr)
			.handle(shutdown_handle)
			.serve(service)
			.await?;
	}
	else {
		let tls_config = config.server.tls_config().await?;
		let hot_reload = tokio::spawn(hot_reload_tls(
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

async fn hot_reload_tls(
	tls_config: RustlsConfig,
	cert_path: impl AsRef<Path>,
	key_path: impl AsRef<Path>,
) -> Result<()> {
	let (mut debouncer, mut event_rx) = tls_watcher().await?;

	debouncer
		.watcher()
		.watch(cert_path.as_ref(), RecursiveMode::NonRecursive)?;
	debouncer
		.watcher()
		.watch(key_path.as_ref(), RecursiveMode::NonRecursive)?;

	while let Some(events) = event_rx.recv().await {
		let should_reload = events.iter().any(|e| {
			let kind = &e.kind;
			kind.is_modify() || kind.is_create() || kind.is_remove()
		});

		if should_reload {
			match tls_config.reload_from_pem_file(&cert_path, &key_path).await {
				Ok(_) => println!("Reloaded TLS certificates"),
				Err(e) => println!("Failed reloading TLS certificates: {e}"),
			};
		}
	}

	Ok(())
}

async fn tls_watcher() -> Result<(
	Debouncer<RecommendedWatcher, FileIdMap>,
	Receiver<Vec<DebouncedEvent>>,
)> {
	let (tx, rx) = tokio::sync::mpsc::channel(1);
	// We're using this since async closures are unstable and I'd rather avoid nightly
	let current_thread = tokio::runtime::Handle::current();

	let debouncer = new_debouncer(Duration::from_secs(1), None, move |res| {
		let tx = tx.clone();

		match res {
			Ok(value) => {
				current_thread.spawn(async move {
					if let Err(e) = tx.send(value).await {
						println!("Failed sending TLS reload event, error: {e}");
					}
				});
			}
			Err(err) => {
				println!("Errored while watching TLS, errors: {err:?}");
			}
		};
	})?;

	Ok((debouncer, rx))
}
