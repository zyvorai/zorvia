//! Golden-image capture: clone a VM's persistent disk into a standalone CDI
//! DataVolume, tracked as an async-shaped job (same synchronous
//! apply-then-report convention as `jobs::DownloadRegistry` -- no real
//! Kubernetes Job/Pod orchestration exists in this codebase, so completion
//! is reported as soon as the clone DataVolume is applied, not once CDI's
//! own import/clone actually finishes).

use crate::kube::cdi::{data_volume_clone_manifest, CdiPvcCloneSpec};
use crate::kube::KubeClient;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConvertStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertJob {
    pub id: String,
    pub status: ConvertStatus,
    pub progress: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
}

/// Why a conversion could not even be started -- distinct from a job that
/// started and then failed (that's reported as a `Failed` `ConvertJob`
/// instead, same as `DownloadRegistry`).
#[derive(Debug)]
pub enum ConvertStartError {
    VmNotFound(String),
    NoPersistentDisk(String),
}

impl std::fmt::Display for ConvertStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VmNotFound(e) => write!(f, "{e}"),
            Self::NoPersistentDisk(e) => write!(f, "{e}"),
        }
    }
}

pub struct ConvertRegistry {
    inner: Mutex<HashMap<String, ConvertJob>>,
}

impl ConvertRegistry {
    pub fn global() -> &'static Self {
        static REG: OnceLock<ConvertRegistry> = OnceLock::new();
        REG.get_or_init(|| ConvertRegistry {
            inner: Mutex::new(HashMap::new()),
        })
    }

    pub fn get(&self, id: &str) -> Option<ConvertJob> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    fn upsert(&self, job: ConvertJob) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(job.id.clone(), job);
    }

    /// Clone `vm_name`'s persistent disk (PVC- or DataVolume-backed) into a
    /// standalone CDI DataVolume named after `image_name`. Ephemeral
    /// `emptyDisk`/`containerDisk`-only VMs have no persistent bytes to
    /// capture and are rejected before any job is created.
    pub async fn start(
        &self,
        vm_name: &str,
        image_name: &str,
        namespace: &str,
        client: &KubeClient,
    ) -> Result<ConvertJob, ConvertStartError> {
        let vm = client.get_vm(namespace, vm_name).await.map_err(|e| {
            ConvertStartError::VmNotFound(format!("VM '{vm_name}' not found: {e}"))
        })?;

        let no_disk_err = || {
            ConvertStartError::NoPersistentDisk(format!(
                "VM '{vm_name}' has no persistent disk to capture (blank/containerdisk disks \
                 are ephemeral) -- golden image capture requires a VM created with a PVC- or \
                 DataVolume-backed disk"
            ))
        };
        let volumes = vm
            .spec
            .template
            .spec
            .volumes
            .as_ref()
            .ok_or_else(no_disk_err)?;

        let (source_pvc, size) = if let Some(vol) = volumes
            .iter()
            .find(|v| v.persistent_volume_claim.is_some())
        {
            let claim_name = vol
                .persistent_volume_claim
                .as_ref()
                .expect("checked is_some above")
                .claim_name
                .clone();
            let size = match client.get_pvc(namespace, &claim_name).await {
                Ok(pvc) => pvc
                    .spec
                    .as_ref()
                    .and_then(|s| s.resources.as_ref())
                    .and_then(|r| r.requests.as_ref())
                    .and_then(|req| req.get("storage"))
                    .map(|q| q.0.clone())
                    .unwrap_or_else(|| "20Gi".into()),
                Err(_) => "20Gi".into(),
            };
            (claim_name, size)
        } else if let Some(vol) = volumes.iter().find(|v| v.data_volume.is_some()) {
            let name = vol
                .data_volume
                .as_ref()
                .expect("checked is_some above")
                .name
                .clone();
            (name, "20Gi".to_string())
        } else {
            return Err(no_disk_err());
        };

        let target_name = slug_image_name(image_name);
        let mut job = ConvertJob {
            id: new_job_id(),
            status: ConvertStatus::Running,
            progress: 0,
            error: None,
            output_path: None,
        };
        self.upsert(job.clone());

        let clone_spec = CdiPvcCloneSpec {
            source_namespace: namespace.to_string(),
            source_pvc,
            target_namespace: namespace.to_string(),
            target_name: target_name.clone(),
            size,
            storage_class: None,
        };

        match data_volume_clone_manifest(&clone_spec) {
            Ok(manifest) => match client.apply_data_volume(namespace, &manifest).await {
                Ok(_) => {
                    job.status = ConvertStatus::Completed;
                    job.progress = 100;
                    // "datavolume:" tells fabric_create_vm's image parser to
                    // attach this disk via add_data_volume_disk, same
                    // convention DownloadRegistry uses for its output_path.
                    job.output_path = Some(format!("datavolume:{target_name}"));
                }
                Err(e) => {
                    job.status = ConvertStatus::Failed;
                    job.error = Some(e.to_string());
                }
            },
            Err(e) => {
                job.status = ConvertStatus::Failed;
                job.error = Some(e.to_string());
            }
        }
        self.upsert(job.clone());
        Ok(job)
    }
}

fn new_job_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(1);
    format!(
        "cv-{}-{}",
        Utc::now().timestamp_millis(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// DNS-label-safe DataVolume name derived from the user-supplied image name.
fn slug_image_name(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
                c
            } else if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(63)
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "golden-image".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_lowercases_and_strips_invalid_chars() {
        assert_eq!(slug_image_name("My Golden_Image!"), "my-golden-image");
    }

    #[test]
    fn slug_falls_back_when_fully_invalid() {
        assert_eq!(slug_image_name("___"), "golden-image");
    }

    #[test]
    fn job_ids_are_unique_and_prefixed() {
        let a = new_job_id();
        let b = new_job_id();
        assert_ne!(a, b);
        assert!(a.starts_with("cv-"));
    }

    #[test]
    fn convert_start_error_displays_message() {
        let err = ConvertStartError::NoPersistentDisk("no disk here".to_string());
        assert_eq!(err.to_string(), "no disk here");
    }
}
