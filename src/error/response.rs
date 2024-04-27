use axum::http::StatusCode;
use axum::Json;
use axum::response::{IntoResponse, Response};
use k8s_openapi::api::core::v1::Pod;
use kube::core::admission::{AdmissionRequest, AdmissionResponse, ConvertAdmissionReviewError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResponseError {
	#[error("Failed converting request into AdmissionRequest for Pod: {0}")]
	ParseAdmission(#[from] ConvertAdmissionReviewError),

	#[error("Pod has no namespace defined (this is unexpected)")]
	NoNamespace,

	#[error(transparent)]
	KubernetesApi(#[from] kube::error::Error),

	#[error("processed pod's namespace {0} doesn't contain a pod-director group label, the MutatingWebhookConfiguration is probably misconfigured", request.namespace.as_ref().unwrap_or(& "unknown".into()))]
	NamespaceMissingLabel { request: AdmissionRequest<Pod> },

	#[error("No pod-director group configured with the name {group}, the namespace {0} is misconfigured", request.namespace.as_ref().unwrap_or(& "unknown".into()))]
	MissingGroupConfig { request: AdmissionRequest<Pod>, group: String },
}

impl IntoResponse for ResponseError {
	fn into_response(self) -> Response {
		match self {
			ResponseError::ParseAdmission(_) => (
				StatusCode::BAD_REQUEST,
				Json(AdmissionResponse::invalid(&self).into_review())
			),
			ResponseError::NoNamespace => (
				StatusCode::BAD_REQUEST,
				Json(AdmissionResponse::invalid(&self).into_review())
			),
			ResponseError::KubernetesApi(_) => (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(AdmissionResponse::invalid(&self).into_review())
			),
			ResponseError::NamespaceMissingLabel { ref request } => {
				let mut response = AdmissionResponse::from(request);
				response.warnings = Some(vec![self.to_string()]);
				(
					StatusCode::OK,
					Json(response.into_review())
				)
			}
			ResponseError::MissingGroupConfig { ref request, .. } => (
				StatusCode::OK,
				Json(AdmissionResponse::from(request).deny(self.to_string()).into_review())
			),
		}.into_response()
	}
}
