mod config;
mod server;
mod error;

#[tokio::main]
async fn main() {
	let config = config::Config::load().unwrap();
	println!("Loaded config:");
	println!("{config:?}");

	match server::serve(&config).await {
		Ok(_) => {}
		Err(e) => {
			println!("ERROR: Failed serving server: {e}");
		}
	};
}
