use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use crate::server::AppState;
use crate::service::KubernetesService;

pub async fn health<S: AppState>(State(app_state): State<S>) -> impl IntoResponse {
	match app_state.kubernetes().healthy().await {
		true => StatusCode::OK,
		false => StatusCode::INTERNAL_SERVER_ERROR,
	}
}

#[cfg(test)]
mod tests {
	use axum::body::Body;
	use axum::http::{Request, StatusCode};
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
	}
}
