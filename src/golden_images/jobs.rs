//! In-memory cloud-image download / CDI import jobs used by the SPA.

use crate::golden_images::{GoldenImageBundle, GoldenImageSpec, ImageSourceType};
use crate::kube::catalog::cloud_images_from_templates;
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    Pending,
    Building,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: String,
    pub name: String,
    pub state: DownloadState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub started: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_volume: Option<serde_json::Value>,
}

pub struct DownloadRegistry {
    inner: Mutex<HashMap<String, DownloadJob>>,
}

impl DownloadRegistry {
    pub fn global() -> &'static Self {
        static REG: OnceLock<DownloadRegistry> = OnceLock::new();
        REG.get_or_init(|| DownloadRegistry {
            inner: Mutex::new(HashMap::new()),
        })
    }

    pub fn list(&self) -> Vec<DownloadJob> {
        let map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut jobs: Vec<_> = map.values().cloned().collect();
        jobs.sort_by(|a, b| b.started.cmp(&a.started));
        jobs
    }

    pub fn get(&self, id: &str) -> Option<DownloadJob> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    pub fn upsert(&self, job: DownloadJob) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(job.id.clone(), job);
    }

    /// Start a download job from a catalog image name, path, or URL: builds
    /// the golden-image DataVolume/DataSource manifests and actually applies
    /// them to the cluster (previously this only built the manifests and
    /// reported "Completed" without ever creating anything — VMs created
    /// from a "downloaded" image would fail since neither the DataVolume nor
    /// the `dv:`-prefixed image reference it returned were ever recognized
    /// anywhere).
    pub async fn start(
        &self,
        requested: &str,
        namespace: &str,
        client: &crate::kube::KubeClient,
    ) -> Result<DownloadJob> {
        let image = resolve_cloud_image(requested)
            .ok_or_else(|| anyhow!("unknown cloud image '{requested}'"))?;
        let now = Utc::now().to_rfc3339();
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let id = format!(
            "dl-{}-{}",
            Utc::now().timestamp_millis(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        );
        let mut job = DownloadJob {
            id: id.clone(),
            name: image.name.clone(),
            state: DownloadState::Building,
            output_path: None,
            error: None,
            started: now,
            completed: None,
            data_volume: None,
        };
        self.upsert(job.clone());

        match build_import_bundle(&image.path, namespace) {
            Ok(bundle) => match client.apply_data_volume(namespace, &bundle.data_volume).await {
                Ok(_) => match client.apply_data_source(namespace, &bundle.data_source).await {
                    Ok(_) => {
                        job.state = DownloadState::Completed;
                        // "datavolume:" tells fabric_create_vm's image parser
                        // to attach this disk via add_data_volume_disk rather
                        // than treating it as an OCI containerdisk reference.
                        job.output_path = Some(format!("datavolume:{}", bundle.versioned_name));
                        job.data_volume = Some(bundle.data_volume);
                    }
                    Err(e) => {
                        job.state = DownloadState::Failed;
                        job.error = Some(format!("failed to create DataSource: {e}"));
                    }
                },
                Err(e) => {
                    job.state = DownloadState::Failed;
                    job.error = Some(format!("failed to create DataVolume: {e}"));
                }
            },
            Err(e) => {
                job.state = DownloadState::Failed;
                job.error = Some(e.to_string());
            }
        }
        job.completed = Some(Utc::now().to_rfc3339());
        self.upsert(job.clone());
        Ok(job)
    }
}

fn resolve_cloud_image(requested: &str) -> Option<crate::kube::catalog::CloudImage> {
    let needle = requested.trim();
    cloud_images_from_templates().into_iter().find(|img| {
        img.name == needle
            || img.path == needle
            || img.url == needle
            || img.template == needle
            || img.name.contains(needle)
    })
}

fn build_import_bundle(path: &str, namespace: &str) -> Result<GoldenImageBundle> {
    let (source_type, source) = if path.starts_with("http://") || path.starts_with("https://") {
        (ImageSourceType::Http, path.to_string())
    } else {
        let url = if path.starts_with("docker://") {
            path.to_string()
        } else {
            format!("docker://{path}")
        };
        (ImageSourceType::Registry, url)
    };
    let slug = path
        .rsplit('/')
        .next()
        .unwrap_or("image")
        .replace([':', '.'], "-")
        .to_ascii_lowercase();
    let spec = GoldenImageSpec {
        name: slug.chars().take(40).collect(),
        version: "import".into(),
        namespace: namespace.to_string(),
        source_type,
        source,
        size: "20Gi".into(),
        storage_class: None,
        checksum: None,
    };
    GoldenImageBundle::build(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    // `DownloadRegistry::start` now actually applies manifests to the
    // cluster, so it needs a real KubeClient and isn't unit-testable here
    // (same as create_vm/hotplug_cpu/etc.) — these tests cover the pure
    // manifest-building and image-resolution logic it depends on instead.

    #[test]
    fn resolves_ubuntu_catalog_entry() {
        let images = cloud_images_from_templates();
        let ubuntu = images
            .iter()
            .find(|i| i.path.contains("ubuntu"))
            .expect("ubuntu image");
        let resolved = resolve_cloud_image(&ubuntu.path).expect("resolves");
        assert_eq!(resolved.path, ubuntu.path);
    }

    #[test]
    fn unknown_image_does_not_resolve() {
        assert!(resolve_cloud_image("not-a-real-image").is_none());
    }

    #[test]
    fn builds_a_versioned_data_volume_and_data_source() {
        let images = cloud_images_from_templates();
        let ubuntu = images
            .iter()
            .find(|i| i.path.contains("ubuntu"))
            .expect("ubuntu image");
        let bundle = build_import_bundle(&ubuntu.path, "default").unwrap();
        assert_eq!(bundle.data_volume["kind"], "DataVolume");
        assert_eq!(bundle.data_volume["metadata"]["name"], bundle.versioned_name);
        assert_eq!(bundle.data_source["kind"], "DataSource");
    }
}
