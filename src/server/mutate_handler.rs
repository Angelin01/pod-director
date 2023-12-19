use crate::config::Config;
use axum::extract::State;
use axum::Json;
use k8s_openapi::api::core::v1::Pod;
use kube::core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview};
use std::sync::Arc;
use kube::core::DynamicObject;

#[axum::debug_handler]
pub async fn mutate_handler(
	State(config): State<Arc<Config>>,
	Json(body): Json<AdmissionReview<Pod>>,
) -> Json<AdmissionReview<DynamicObject>> {
	let request: AdmissionRequest<Pod> = match body.try_into() {
		Err(err) => {
			return Json(AdmissionResponse::invalid(err).into_review());
		}
		Ok(v) => v
	};

	println!("${request:?}");
	Json(AdmissionResponse::from(&request).into_review())
}
