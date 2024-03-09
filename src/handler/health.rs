use axum::Json;
use axum::response::IntoResponse;
use serde::Serialize;

pub async fn health() -> impl IntoResponse {
	Json(HealthResponse { status: "ok".into() })
}

#[derive(Serialize)]
struct HealthResponse {
	status: String,
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use axum::body::Body;
	use axum::http::{Request, StatusCode};
	use axum::Router;
	use axum::routing::get;
	use http_body_util::BodyExt;
	use serde_json::{json, Value};
	use tower::ServiceExt;

	use crate::handler;
	use crate::config::Config;

	#[tokio::test]
	async fn health_test() {
		let config = Arc::new(Config::default());
		let app = Router::new()
			.route("/health", get(handler::health));

		let request = Request::builder().uri("/health").body(Body::empty()).unwrap();

		let response = app
			.oneshot(request)
			.await
			.unwrap();

		assert_eq!(response.status(), StatusCode::OK);
		let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
		let body: Value = serde_json::from_slice(&body_bytes).unwrap();
		assert_eq!(body, json!({"status": "ok"}));
	}
}
