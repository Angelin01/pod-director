use axum::http::StatusCode;
use axum::response::Response;
use kube::api::DynamicObject;
use kube::core::admission::{AdmissionResponse, AdmissionReview};
use http_body_util::BodyExt;

pub struct ParsedResponse {
	pub status: StatusCode,
	pub admission_response: AdmissionResponse,
}

impl ParsedResponse {
	pub async fn from_response(response: Response) -> Self {
		let status = response.status();
		let response_bytes = response.into_body().collect().await.unwrap().to_bytes();
		let parsed_review: AdmissionReview<DynamicObject> = serde_json::from_slice(&response_bytes).unwrap();
		let admission_response = parsed_review.response.unwrap();

		ParsedResponse {
			status,
			admission_response,
		}
	}
}
