use axum::Json;
use serde::Serialize;

pub async fn health_handler() -> Json<HealthResponse> {
	Json(HealthResponse { status: "ok".into() })
}

#[derive(Serialize)]
pub struct HealthResponse {
	status: String,
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use axum::body::Body;
	use axum::http::{Request, StatusCode};
	use serde_json::{json, Value};
	use tower::ServiceExt;
	use http_body_util::BodyExt;
	use crate::config::Config;
	use crate::server;

	#[tokio::test]
	async fn health_test() {
		let config = Arc::new(Config::default());
		let app = server::build_app(config);

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
