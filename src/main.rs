mod config;

fn main() {
	let config = config::Config::load();
	println!("Loaded config:");
	println!("{config:?}");
}
