mod health_handler;

use axum::Router;
use axum::routing::get;
use health_handler::health_handler;

pub fn build_app() -> Router {
	Router::new()
		.route("/health", get(health_handler))
}
