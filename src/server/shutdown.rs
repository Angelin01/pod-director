use axum_server::Handle;
use tokio::signal;
use std::time::Duration;

pub async fn graceful_shutdown(handle: Handle) {
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
