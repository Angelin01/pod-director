mod config;
mod server;

#[tokio::main]
async fn main() {
	let config = config::Config::load().unwrap();
	println!("Loaded config:");
	println!("{config:?}");

	axum::Server::bind(&config.server.socker_addr())
		.serve(server::build_app().into_make_service())
		.await
		.unwrap();
}
