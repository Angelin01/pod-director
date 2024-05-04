use axum::extract::State;
use axum::Json;
use axum::response::Result;
use k8s_openapi::api::core::v1::Pod;
use kube::api::DynamicObject;
use kube::core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview};

use crate::error::ResponseError;
use crate::server::AppState;
use crate::service::KubernetesService;
use crate::utils::patch;
use crate::utils::patch::PatchResult;

pub async fn mutate<S: AppState>(
	State(app_state): State<S>,
	Json(body): Json<AdmissionReview<Pod>>,
) -> Result<Json<AdmissionReview<DynamicObject>>, ResponseError> {
	let request: AdmissionRequest<Pod> = body.try_into()?;

	let namespace = request.namespace.as_ref().ok_or(ResponseError::NoNamespace)?;

	let group = match app_state.kubernetes().namespace_group(namespace).await? {
		Some(g) => g,
		None => return Err(ResponseError::NamespaceMissingLabel {
			request,
		}),
	};

	let group_config = match app_state.config().groups.get(&group) {
		Some(group_config) => group_config,
		None => return Err(ResponseError::MissingGroupConfig {
			request,
			group,
		}),
	};

	let mut patches = Vec::new();
	let pod = request.object.as_ref().expect("Request object is missing");

	if let Some(node_selector_config) = &group_config.node_selector {
		let node_selector_patches = patch::calculate_node_selector_patches(
			pod,
			node_selector_config,
			&group_config.on_conflict,
		);

		match node_selector_patches {
			PatchResult::Allow(v) => patches.extend(v),
			PatchResult::Deny { label, config_value, conflicting_value } => {
				let reason = format!(
					"The pod's nodeSelector {label}={conflicting_value} conflicts with pod-director's configuration {label}={config_value}"
				);
				return Ok(Json(AdmissionResponse::from(&request).deny(reason).into_review()));
			}
		}
	}

	if let Some(tolerations_config) = &group_config.tolerations {
		let toleration_patches = patch::calculate_toleration_patches(pod, tolerations_config);
		patches.extend(toleration_patches);
	}

	Ok(Json(
		AdmissionResponse::from(&request)
			.with_patch(json_patch::Patch(patches))
			.unwrap()
			.into_review(),
	))
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use axum::body::Body;
	use axum::http::{Request, StatusCode};
	use axum::response::Response;
	use k8s_openapi::api::core::v1::Toleration;
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
			patch::add("/spec/nodeSelector/some-label".into(), "some-value".into()),
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

	#[tokio::test]
	async fn when_fails_fetching_label_for_namespace_should_return_internal_server_error() {
		let mut state = TestAppState::new(Config::default());
		state.kubernetes.set_error();

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::INTERNAL_SERVER_ERROR);
	}

	#[tokio::test]
	async fn when_pod_has_no_tolerations_should_insert_tolerations_and_pd_tolerations() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: None,
			affinity: None,
			tolerations: Some(vec![Toleration {
				key: Some("some-key".into()),
				value: Some("some-value".into()),
				operator: Some("Equals".into()),
				effect: Some("NoSchedule".into()),
				toleration_seconds: None,
			}]),
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
			patch::add("/spec/tolerations".into(), json!([])),
			patch::add("/spec/tolerations/-".into(), json!({
				"key": "some-key",
				"value": "some-value",
				"operator": "Equals",
				"effect": "NoSchedule"
			})),
		];

		assert_eq!(result.patches, expected_patches);
	}

	#[tokio::test]
	async fn when_pod_has_existing_tolerations_not_matching_config_should_only_insert_pd_tolerations() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: None,
			affinity: None,
			tolerations: Some(vec![Toleration {
				key: Some("some-key".into()),
				value: Some("some-value".into()),
				operator: Some("Equals".into()),
				effect: Some("NoSchedule".into()),
				toleration_seconds: None,
			}]),
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_toleration(Toleration {
				effect: Some("NoExecute".into()),
				key: Some("other".into()),
				operator: Some("Exists".into()),
				toleration_seconds: None,
				value: None,
			})
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		let expected_patches = vec![
			patch::add("/spec/tolerations/-".into(), json!({
				"key": "some-key",
				"value": "some-value",
				"operator": "Equals",
				"effect": "NoSchedule"
			})),
		];

		assert_eq!(result.patches, expected_patches);
	}

	#[tokio::test]
	async fn when_pod_has_existing_tolerations_with_some_matching_config_should_only_insert_necessary_tolerations() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: None,
			affinity: None,
			tolerations: Some(vec![
				Toleration {
					key: Some("some-key".into()),
					value: Some("some-value".into()),
					operator: Some("Equals".into()),
					effect: Some("NoSchedule".into()),
					toleration_seconds: None,
				},
				Toleration {
					effect: Some("NoExecute".into()),
					key: Some("other".into()),
					operator: Some("Exists".into()),
					toleration_seconds: None,
					value: None,
				},
			]),
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_toleration(Toleration {
				effect: Some("NoExecute".into()),
				key: Some("other".into()),
				operator: Some("Exists".into()),
				toleration_seconds: None,
				value: None,
			})
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);

		let expected_patches = vec![
			patch::add("/spec/tolerations/-".into(), json!({
				"key": "some-key",
				"value": "some-value",
				"operator": "Equals",
				"effect": "NoSchedule"
			})),
		];

		assert_eq!(result.patches, expected_patches);
	}

	#[tokio::test]
	async fn when_pod_has_existing_tolerations_with_perfect_matching_config_should_should_do_nothing() {
		let mut config = Config::default();
		let group_config = GroupConfig {
			node_selector: None,
			affinity: None,
			tolerations: Some(vec![
				Toleration {
					key: Some("some-key".into()),
					value: Some("some-value".into()),
					operator: Some("Equals".into()),
					effect: Some("NoSchedule".into()),
					toleration_seconds: None,
				},
				Toleration {
					effect: Some("NoExecute".into()),
					key: Some("other".into()),
					operator: Some("Exists".into()),
					toleration_seconds: None,
					value: None,
				},
			]),
			on_conflict: Conflict::Reject,
		};
		config.groups = HashMap::from([
			("bar".into(), group_config)
		]);

		let mut state = TestAppState::new(config);
		state.kubernetes.set_namespace_group("foo", "bar");

		let body = PodCreateRequestBuilder::new()
			.with_namespace("foo")
			.with_toleration(Toleration {
				effect: Some("NoExecute".into()),
				key: Some("other".into()),
				operator: Some("Exists".into()),
				toleration_seconds: None,
				value: None,
			}).
			with_toleration(Toleration {
				key: Some("some-key".into()),
				value: Some("some-value".into()),
				operator: Some("Equals".into()),
				effect: Some("NoSchedule".into()),
				toleration_seconds: None,
			})
			.build();

		let response = mutate_request(state, body).await;
		let result = ParsedResponse::from_response(response).await;
		assert_eq!(result.status, StatusCode::OK);
		assert_eq!(result.admission_response.allowed, true);
		assert!(result.patches.is_empty());
	}
}
