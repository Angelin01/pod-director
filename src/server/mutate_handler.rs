use crate::config::Config;
use crate::service::kubernetes;
use axum::extract::State;
use axum::Json;
use k8s_openapi::api::core::v1::Pod;
use kube::core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview};
use kube::core::DynamicObject;
use std::sync::Arc;

#[axum::debug_handler]
pub async fn mutate_handler(
	State(config): State<Arc<Config>>,
	Json(body): Json<AdmissionReview<Pod>>,
) -> Json<AdmissionReview<DynamicObject>> {
	let request: AdmissionRequest<Pod> = match body.try_into() {
		Err(err) => {
			// TODO: should probably be bad request
			return Json(AdmissionResponse::invalid(err).into_review());
		}
		Ok(v) => v,
	};

	let namespace = match request.namespace {
		None => {
			return Json(
				AdmissionResponse::invalid("Pod has no namespace defined (this is unexpected)")
					.into_review(),
			);
		}
		Some(ref v) => v,
	};

	let group = match kubernetes::namespace_group(namespace).await {
		Err(err) => {
			// TODO: should be internal server error
			return Json(AdmissionResponse::invalid(err).into_review());
		}
		Ok(v) => match v {
			None => {
				let warnings = vec!["processed pod's namespace doesn't contain a pod-director group label, the MutatingWebhookConfiguration is probably misconfigured".into()];
				let mut response = AdmissionResponse::from(&request);
				response.warnings = Some(warnings);
				return Json(response.into_review());
			}
			Some(g) => g,
		},
	};

	println!("In namespace {namespace}, group {group:?}");

	Json(AdmissionResponse::from(&request).into_review())
}
