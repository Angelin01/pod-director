use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use serde::Serialize;
use crate::server::AppState;
use crate::service::KubernetesService;

pub async fn health<S: AppState>(State(app_state): State<S>) -> impl IntoResponse {
	match app_state.kubernetes().healthy().await {
		true => (StatusCode::OK, Json(HealthResponse { status: "ok".into() })),
		false => (StatusCode::INTERNAL_SERVER_ERROR, Json(HealthResponse { status: "err".into() })),
	}
}

#[derive(Serialize)]
struct HealthResponse {
	status: String,
}

#[cfg(test)]
mod tests {
	use axum::body::Body;
	use axum::http::{Request, StatusCode};
	use http_body_util::BodyExt;
	use serde_json::{json, Value};
	use tower::ServiceExt;

	use crate::config::Config;
	use crate::server;
	use crate::server::state::tests::TestAppState;

	#[tokio::test]
	async fn health_ok_test() {
		let app = server::build_app(TestAppState::new(Config::default()));

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

	#[tokio::test]
	async fn health_err_test() {
		let mut app_state = TestAppState::new(Config::default());
		app_state.kubernetes.set_error(true);
		let app = server::build_app(app_state);

		let request = Request::builder().uri("/health").body(Body::empty()).unwrap();

		let response = app
			.oneshot(request)
			.await
			.unwrap();

		assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
		let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
		let body: Value = serde_json::from_slice(&body_bytes).unwrap();
		assert_eq!(body, json!({"status": "err"}));
	}
}
