use axum::Json;
use k8s_openapi::api::core::v1::Pod;
use kube::core::admission::{AdmissionReview};

#[axum::debug_handler]
pub async fn mutate_handler(Json(body): Json<AdmissionReview<Pod>>) -> Json<AdmissionReview<Pod>> {
	Json(body)
}
