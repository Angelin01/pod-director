use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use axum::async_trait;
use k8s_openapi::api::core::v1::Namespace;
use kube::{Api, Client, ResourceExt};
use kube::runtime::{reflector, watcher, WatchStreamExt};
use kube::runtime::reflector::{ObjectRef, Store};
use futures::{future, Stream, StreamExt};
use kube::runtime::reflector::store::Writer;

#[async_trait]
pub trait KubernetesService: Send + Sync + Clone {
    async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace: S) -> Option<String>;

    async fn healthy(&self) -> bool;
}

#[derive(Clone)]
pub struct StandardKubernetesService {
    store: Store<Namespace>,
    group_label: String,
    healthy: Arc<AtomicBool>,
}

impl StandardKubernetesService {
    pub async fn new<S: AsRef<str>>(group_label: S) -> anyhow::Result<Self> {
        let api: Api<Namespace> = Api::all(Client::try_default().await?);
        // TODO: Map errors to healthcheck
        let watcher = kube::runtime::watcher(api, Default::default());

        let (reader, writer) = kube::runtime::reflector::store();

        let healthy = Arc::new(AtomicBool::new(true));

        Self::watch_namespaces(watcher, writer, &healthy);

        reader.wait_until_ready().await?;

        Ok(StandardKubernetesService {
            store: reader,
            group_label: group_label.as_ref().to_string(),
            healthy,
        })
    }

    fn watch_namespaces(
        watcher: impl Stream<Item=watcher::Result<watcher::Event<Namespace>>> + Send + Sized + 'static,
        writer: Writer<Namespace>,
        healthy: &Arc<AtomicBool>,
    ) {
        let healthy_clone = Arc::clone(&healthy);
        let stream = reflector(writer, watcher)
            .default_backoff()
            .touched_objects()
            .for_each(move |r| {
                let reflector_healthy = Arc::clone(&healthy_clone);
                future::ready(match r {
                    Ok(_) => {
                        reflector_healthy.store(true, Ordering::Relaxed);
                    }
                    Err(e) => {
                        reflector_healthy.store(false, Ordering::Relaxed);
                        println!("watcher error: {e}")
                    }
                })
            });
        tokio::spawn(stream);
    }
}

#[async_trait]
impl KubernetesService for StandardKubernetesService {
    async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace_name: S) -> Option<String> {
        let namespace_ref= &ObjectRef::<Namespace>::new(namespace_name.as_ref());
        let namespace = match self.store.get(namespace_ref) {
            None => return None,
            Some(n) => n,
        };

        let result = match namespace.labels().get(&self.group_label) {
            None => None,
            Some(s) => Some(s.to_string())
        };

        result
    }

    async fn healthy(&self) -> bool { self.healthy.load(Ordering::Relaxed) }
}

#[cfg(test)]
pub mod tests {
    use std::collections::BTreeMap;
    use axum::async_trait;
    use crate::service::kubernetes::KubernetesService;

    #[derive(Clone)]
    pub struct MockKubernetesService {
        namespace_group_map: BTreeMap<String, String>,
        is_error: bool,
    }

    impl MockKubernetesService {
        pub fn new() -> Self {
            MockKubernetesService {
                namespace_group_map: BTreeMap::new(),
                is_error: false,
            }
        }

        pub fn set_namespace_group<S: AsRef<str>, R: AsRef<str>>(&mut self, namespace: S, group: R) {
            self.namespace_group_map.insert(namespace.as_ref().into(), group.as_ref().into());
        }

        pub fn set_error(&mut self, is_erroring: bool) {
            self.is_error = is_erroring;
        }
    }

    #[async_trait]
    impl KubernetesService for MockKubernetesService {
        async fn namespace_group<S: AsRef<str> + Send + Sync>(&self, namespace: S) -> Option<String> {
            self.namespace_group_map.get(namespace.as_ref()).map(String::to_owned)
        }

        async fn healthy(&self) -> bool { !self.is_error }
    }
}
