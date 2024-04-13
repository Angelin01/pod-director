use axum_server::tls_rustls::RustlsConfig;
use std::path::Path;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{DebouncedEvent, Debouncer, FileIdMap, new_debouncer};
use tokio::sync::mpsc::Receiver;
use std::time::Duration;

pub async fn hot_reload_tls(
	tls_config: RustlsConfig,
	cert_path: impl AsRef<Path>,
	key_path: impl AsRef<Path>,
) -> anyhow::Result<()> {
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

async fn tls_watcher() -> anyhow::Result<(
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
