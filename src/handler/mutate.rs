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
				let warning = format!("processed pod's namespace {namespace} doesn't contain a pod-director group label, the MutatingWebhookConfiguration is probably misconfigured");
				let mut response = AdmissionResponse::from(&request);
				response.warnings = Some(vec![warning]);
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
	conflict_config: &Conflict,
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
				if let Some(existing_value) = node_selector.get(k) {
					if existing_value == v {
						continue
					}

					match conflict_config {
						Conflict::Ignore => (),
						Conflict::Override => {
							patches.push(patch::replace(format!("/spec/nodeSelector/{k}"), json!(v)));
						}
						Conflict::Reject => {
							break;
						}
					}
				}
				else {
					patches.push(patch::add(format!("/spec/nodeSelector/{k}"), json!(v)));
				}
			}
		}
	}

	patches
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;
	use axum::body::Body;
	use axum::http::{Request, StatusCode};
	use axum::response::Response;
	use serde_json::json;
	use tower::ServiceExt;

	use crate::config::{Config, Conflict, GroupConfig};
	use crate::server;
	use crate::server::state::tests::TestAppState;
	use crate::test_utils::{ParsedResponse, PodCreateRequestBuilder};
	use crate::utils::patch;

	async fn mutate_request(state: TestAppState, body: Body) -> Response {
		let request = Request::builder()
			.uri("/mutate")
			.header("Content-Type", "application/json")
			.method("POST")
			.body(body)
			.unwrap();

		server::build_app(state)
			.oneshot(request)
			.await
			.unwrap()
	}

	#[tokio::test]
	async fn when_pod_namespace_has_no_pd_label_should_allow_with_warning() {
		let state = TestAppState::new(Config::default());
		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.build();

		let response = mutate_request(state, body).await;

		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);
		assert_eq!(result.admission_response.warnings, Some(vec!["processed pod's namespace foo doesn't contain a pod-director group label, the MutatingWebhookConfiguration is probably misconfigured".to_owned()]))
	}

	#[tokio::test]
	async fn when_namespace_config_does_not_match_any_group_should_deny_pod() {
		let mut state = TestAppState::new(Config::default());
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.build();

		let response = mutate_request(state, body).await;

		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, false);
		assert_eq!(
			result.admission_response.result.message,
			"No pod-director group configured with the name bar, the namespace foo is misconfigured"
		)
	}

	#[tokio::test]
	async fn when_pod_has_no_node_selector_should_insert_node_selector_and_pd_labels() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("some-label".into(), "some-value".into())
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		let expected_patches = vec![
			patch::add("/spec/nodeSelector".into(), json!({})),
			patch::add("/spec/nodeSelector/some-label".into(), "some-value".into())
		];

		assert_eq!(result.patches, expected_patches);
	}

	#[tokio::test]
	async fn when_pod_has_existing_node_selector_not_matching_config_should_only_insert_pd_labels() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("some-label".into(), "some-value".into())
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_node_selector("existing-label", "existing-value")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		let expected_patches = vec![
			patch::add("/spec/nodeSelector/some-label".into(), "some-value".into())
		];

		assert_eq!(result.patches, expected_patches);
	}

	#[tokio::test]
	async fn when_pod_has_existing_node_selector_with_some_matching_config_should_only_insert_necessary_labels() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("label-0".into(), "value-0".into()),
				("label-1".into(), "value-1".into()),
				("label-2".into(), "value-2".into())
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_node_selector("label-1", "value-1")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		assert!(result.patches.contains(&patch::add("/spec/nodeSelector/label-0".into(), "value-0".into())));
		assert!(result.patches.contains(&patch::add("/spec/nodeSelector/label-2".into(), "value-2".into())));
	}

	#[tokio::test]
	async fn when_pod_has_existing_node_selector_with_perfect_matching_config_should_do_nothing() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("label-0".into(), "value-0".into()),
				("label-1".into(), "value-1".into()),
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_node_selector("label-0", "value-0")
			.with_node_selector("label-1", "value-1")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		assert!(result.patches.is_empty());
	}

	#[tokio::test]
	async fn when_pod_has_existing_node_selector_with_matching_config_and_extra_labels_should_do_nothing() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("label-0".into(), "value-0".into()),
				("label-1".into(), "value-1".into()),
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_node_selector("label-0", "value-0")
			.with_node_selector("label-1", "value-1")
			.with_node_selector("label-2", "value-2")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		assert!(result.patches.is_empty());
	}

	#[tokio::test]
	async fn when_pod_has_conflicting_node_selector_and_config_is_ignore_should_ignore_label() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("label-0".into(), "value-0".into()),
				("label-1".into(), "value-1".into()),
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Ignore,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_node_selector("label-0", "conflicting-value")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		let expected_patches = vec![
			patch::add("/spec/nodeSelector/label-1".into(), "value-1".into())
		];

		assert_eq!(result.patches, expected_patches);
	}

	#[tokio::test]
	async fn when_pod_has_conflicting_node_selector_and_config_is_override_should_replace_label() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("label-0".into(), "value-0".into()),
				("label-1".into(), "value-1".into()),
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Override,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_node_selector("label-0", "conflicting-value")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		assert!(result.patches.contains(&patch::replace("/spec/nodeSelector/label-0".into(), "value-0".into())));
		assert!(result.patches.contains(&patch::add("/spec/nodeSelector/label-1".into(), "value-1".into())));
	}

	#[tokio::test]
	async fn when_pod_has_conflicting_node_selector_and_config_is_reject_should_reject_pod() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: Some(HashMap::from([
				("label-0".into(), "value-0".into()),
				("label-1".into(), "value-1".into()),
			])),
			affinity: None,
			tolerations: None,
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_node_selector("label-0", "conflicting-value")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, false);
		assert_eq!(
			result.admission_response.result.message,
			"The pod's nodeSelector label-0=conflicting-value conflicts with pod-director's configuration label-0=value-0"
		);
	}

	// TODO: test conflicts
	// TODO: test k8s error
}
