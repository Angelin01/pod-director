use std::sync::Arc;

mod config;
mod error;
mod server;
mod service;
mod utils;

#[tokio::main]
async fn main() {
	let config = Arc::new(config::Config::load().unwrap());
	println!("Loaded config:");
	println!("{config:?}");

	match server::serve(config).await {
		Ok(_) => {}
		Err(e) => {
			println!("ERROR: Failed serving server: {e}");
		}
	};
}
