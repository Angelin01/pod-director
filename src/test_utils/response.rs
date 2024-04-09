use axum::http::StatusCode;
use axum::response::Response;
use http_body_util::BodyExt;
use json_patch::{Patch, PatchOperation};
use kube::api::DynamicObject;
use kube::core::admission::{AdmissionResponse, AdmissionReview};

pub struct ParsedResponse {
	pub status: StatusCode,
	pub admission_response: AdmissionResponse,
	pub patches: Vec<PatchOperation>,
}

impl ParsedResponse {
	pub async fn from_response(response: Response) -> Self {
		let status = response.status();
		let response_bytes = response.into_body().collect().await.unwrap().to_bytes();
		let parsed_review: AdmissionReview<DynamicObject> = serde_json::from_slice(&response_bytes).unwrap();
		let admission_response = parsed_review.response.unwrap();
		let patches = admission_response.patch.as_ref()
			.map_or_else(Vec::new, |v| serde_json::from_slice::<Patch>(v).unwrap().0);

		ParsedResponse {
			status,
			admission_response,
			patches,
		}
	}
}
