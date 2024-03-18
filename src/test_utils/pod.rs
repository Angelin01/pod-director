use std::collections::BTreeMap;
use axum::body::Body;
use serde_json::{json, Value};

pub struct PodCreateRequestBuilder {
	namespace: Option<String>,
	node_selector: Option<BTreeMap<String, String>>,
}

impl PodCreateRequestBuilder {
	pub fn new() -> Self {
		Self { namespace: None, node_selector: None }
	}

	pub fn with_namespace<S: AsRef<str>>(mut self, namespace: S) -> Self {
		self.namespace = Some(namespace.as_ref().to_string());
		self
	}

	pub fn with_node_selector<S: AsRef<str>, R: AsRef<str>>(mut self, label: S, value: R) -> Self {
		self.node_selector.get_or_insert_with(BTreeMap::new)
			.insert(label.as_ref().into(), value.as_ref().into());
		self
	}

	pub fn build(self) -> Body {
		let data = json!({
		  "apiVersion": "admission.k8s.io/v1",
		  "kind": "AdmissionReview",
		  "request": {
		    "uid": "354be64e-f80a-49be-9b14-d7a5acae507b",
		    "kind": {
		      "group": "",
		      "version": "v1",
		      "kind": "Pod"
		    },
		    "resource": {
		      "group": "",
		      "version": "v1",
		      "resource": "pods",
		      "api_version": ""
		    },
		    "subResource": null,
		    "requestKind": {
		      "group": "",
		      "version": "v1",
		      "kind": "Pod"
		    },
		    "requestResource": {
		      "group": "",
		      "version": "v1",
		      "resource": "pods",
		      "api_version": ""
		    },
		    "requestSubResource": null,
		    "name": "test",
		    "namespace": self.namespace,
		    "operation": "CREATE",
		    "userInfo": {
		      "groups": [
		        "system:masters",
		        "system:authenticated"
		      ],
		      "username": "user"
		    },
		    "object": {
		      "apiVersion": "v1",
		      "kind": "Pod",
		      "metadata": {
		        "labels": {
		          "run": "test"
		        },
		        "managedFields": [],
		        "name": "test",
		        "namespace": "test"
		      },
		      "spec": {
		      "containers": [{
		        "args": ["sh"],
		        "image": "alpine",
		        "imagePullPolicy": "Always",
		        "name": "test",
		        "resources": {},
		        "stdin": true,
		        "stdinOnce": true,
		        "terminationMessagePath": "/dev/termination-log",
		        "terminationMessagePolicy": "File",
		        "tty": true,
		        "volumeMounts": [{
		          "mountPath": "/var/run/secrets/kubernetes.io/serviceaccount",
		          "name": "kube-api-access-vqj85",
		          "readOnly": true
		        }]
		      }],
		      "nodeSelector": self.node_selector,
		      "dnsPolicy": "ClusterFirst",
		      "enableServiceLinks": true,
		      "preemptionPolicy": "PreemptLowerPriority",
		      "priority": 0,
		      "restartPolicy": "Always",
		      "schedulerName": "default-scheduler",
		      "securityContext": {},
		      "serviceAccount": "default",
		      "serviceAccountName": "default",
		      "terminationGracePeriodSeconds": 30,
		      "tolerations": [
		        {
		          "effect": "NoExecute",
		          "key": "node.kubernetes.io/not-ready",
		          "operator": "Exists",
		          "tolerationSeconds": 300
		        },
		        {
		          "effect": "NoExecute",
		          "key": "node.kubernetes.io/unreachable",
		          "operator": "Exists",
		          "tolerationSeconds": 300
		        }
		      ],
		      "volumes": []
		      },
		      "status": {}
		    },
		    "oldObject": null,
		    "dryRun": false,
		    "options": {
		      "apiVersion": "meta.k8s.io/v1",
		      "fieldManager": "kubectl-run",
		      "kind": "CreateOptions"
		    }
		  }
		});

		Body::from(serde_json::to_vec(&data).unwrap())
	}
}
