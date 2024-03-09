use std::collections::HashMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum::response::IntoResponse;
use json_patch::PatchOperation;
use k8s_openapi::api::core::v1::Pod;
use kube::core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview};
use serde_json::json;

use crate::config::Conflict;
use crate::server::AppState;
use crate::service::kubernetes::KubernetesService;
use crate::utils::patch;

pub async fn mutate<S: AppState>(
	State(app_state): State<S>,
	Json(body): Json<AdmissionReview<Pod>>,
) -> impl IntoResponse {
	let request: AdmissionRequest<Pod> = match body.try_into() {
		Err(err) => {
			// TODO: Should probably have a custom error return
			println!("Bad request");
			return (
				StatusCode::BAD_REQUEST,
				Json(AdmissionResponse::invalid(err).into_review()),
			);
		}
		Ok(v) => v,
	};

	let namespace = match request.namespace {
		None => {
			return (
				StatusCode::OK,
				Json(
					AdmissionResponse::invalid("Pod has no namespace defined (this is unexpected)")
						.into_review(),
				),
			);
		}
		Some(ref v) => v,
	};

	let group = match app_state.kubernetes().namespace_group(namespace).await {
		Err(err) => {
			// TODO: Should probably have a custom error return
			println!("Couldn't get namespace: {err:?}");
			return (
				StatusCode::INTERNAL_SERVER_ERROR,
				Json(AdmissionResponse::from(&request).into_review()),
			);
		}
		Ok(v) => match v {
			None => {
				let warnings = vec!["processed pod's namespace doesn't contain a pod-director group label, the MutatingWebhookConfiguration is probably misconfigured".into()];
				let mut response = AdmissionResponse::from(&request);
				response.warnings = Some(warnings);
				return (StatusCode::OK, Json(response.into_review()));
			}
			Some(g) => g,
		},
	};

	let mut patches = Vec::new();
	match &app_state.config().groups.get(&group) {
		None => {
			let reason = format!(
				"No pod-director group configured with the name {group}, the namespace {namespace} is misconfigured"
			);

			return (
				StatusCode::OK,
				Json(AdmissionResponse::from(&request).deny(reason).into_review()),
			);
		}
		Some(ref group_config) => {
			if let Some(node_selector_config) = &group_config.node_selector {
				patches.extend(calculate_node_selector_patches(
					request.object.as_ref().unwrap(),
					&node_selector_config,
					&group_config.on_conflict,
				));
			}
		}
	};

	println!("In namespace {namespace}, group {group:?}");

	(
		StatusCode::OK,
		Json(
			AdmissionResponse::from(&request)
				.with_patch(json_patch::Patch(patches))
				.unwrap()
				.into_review(),
		),
	)
}

fn calculate_node_selector_patches(
	pod: &Pod,
	node_selector_config: &HashMap<String, String>,
	conflict_config: &Conflict
) -> Vec<PatchOperation> {
	let mut patches = Vec::new();

	let maybe_node_selector = &pod.spec.as_ref().unwrap().node_selector;

	match maybe_node_selector.as_ref() {
		None => {
			patches.push(patch::add("/spec/nodeSelector".into(), json!({})));
			node_selector_config.iter().for_each(|(k, v)| {
				patches.push(patch::add(format!("/spec/nodeSelector/{k}"), json!(v)));
			})
		}
		Some(node_selector) => {
			for (k, v) in node_selector_config.iter() {
				if let Some(_) = node_selector.get(k) {
					match conflict_config {
						Conflict::Ignore => {
							println!("Conflict found! As 'on_conflict' is set to 'ignore', the original value will be kept.");
						}
						Conflict::Override => {
							println!("Conflict found! As 'on_conflict' is set to 'override', the original value will be overriden.");
							patches.push(patch::replace(format!("/spec/nodeSelector/{k}"), json!(v)));
						}
						Conflict::Reject => {
							println!("Conflict found! As 'on_conflict' is set to 'refuse', the entire operation will halt.");
							break;
						}
					}
				}
			}
		}
	}

	patches
}
