//! Phase 5 enterprise capability plans (gated by `ZORVIA_EXPERIMENTAL` + feature flags).
//!
//! These produce actionable plans / dry-runs. They do not call AWS, vSphere, or
//! remote clusters until a later shipping release.

use serde::{Deserialize, Serialize};

fn require(flag: &str) -> Result<(), String> {
    if crate::features::enterprise_flag(flag) {
        Ok(())
    } else {
        Err(format!(
            "Enterprise feature disabled. Set ZORVIA_EXPERIMENTAL=1 and {flag}=1"
        ))
    }
}

/// S3 immutable backup export plan (no network I/O).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3ImmutableBackupPlan {
    pub vm_name: String,
    pub snapshot_name: String,
    pub bucket: String,
    pub region: String,
    pub object_key: String,
    pub object_lock_mode: String,
    pub retention_days: u32,
    pub notes: Vec<String>,
}

impl S3ImmutableBackupPlan {
    pub fn try_plan(
        vm_name: impl Into<String>,
        snapshot_name: impl Into<String>,
        bucket: impl Into<String>,
        region: impl Into<String>,
        prefix: impl Into<String>,
        retention_days: u32,
    ) -> Result<Self, String> {
        require("ZORVIA_FEATURE_S3_BACKUP")?;
        let vm_name = vm_name.into();
        let snapshot_name = snapshot_name.into();
        let bucket = bucket.into();
        let region = region.into();
        let prefix = prefix.into().trim_matches('/').to_string();
        let object_key = format!(
            "{}/{}/{}.tar.zst",
            if prefix.is_empty() {
                "zorvia-backups".into()
            } else {
                prefix
            },
            vm_name,
            snapshot_name
        );
        Ok(Self {
            vm_name,
            snapshot_name,
            bucket,
            region,
            object_key,
            object_lock_mode: "COMPLIANCE".into(),
            retention_days: retention_days.max(1),
            notes: vec![
                "Plan only — VolumeSnapshot bytes are not uploaded in this release.".into(),
                "Enable S3 Object Lock (compliance) on the bucket before first write.".into(),
                "Wire Velero/Kopia or a custom exporter to execute this plan.".into(),
            ],
        })
    }
}

/// Transiva VMware → KubeVirt conversion plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransivaPlan {
    pub source_vm: String,
    pub vcenter_endpoint: String,
    pub target_namespace: String,
    pub stages: Vec<String>,
    pub notes: Vec<String>,
}

impl TransivaPlan {
    pub fn try_plan(
        source_vm: impl Into<String>,
        vcenter_endpoint: impl Into<String>,
        target_namespace: impl Into<String>,
    ) -> Result<Self, String> {
        require("ZORVIA_FEATURE_TRANSIVA")?;
        Ok(Self {
            source_vm: source_vm.into(),
            vcenter_endpoint: vcenter_endpoint.into(),
            target_namespace: target_namespace.into(),
            stages: vec![
                "discover".into(),
                "convert".into(),
                "import".into(),
                "validate".into(),
            ],
            notes: vec![
                "Plan only — Transiva agent/API integration is not shipped yet.".into(),
                "Ensure network reachability from the conversion worker to vCenter.".into(),
            ],
        })
    }
}

/// Golden-image build → scan → sign → promote pipeline plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenPipelinePlan {
    pub image_name: String,
    pub version: String,
    pub namespace: String,
    pub stages: Vec<String>,
    pub notes: Vec<String>,
}

impl GoldenPipelinePlan {
    pub fn try_plan(
        image_name: impl Into<String>,
        version: impl Into<String>,
        namespace: impl Into<String>,
    ) -> Result<Self, String> {
        require("ZORVIA_FEATURE_GOLDEN_PIPELINE")?;
        let image_name = image_name.into();
        let version = version.into();
        Ok(Self {
            image_name: image_name.clone(),
            version: version.clone(),
            namespace: namespace.into(),
            stages: vec![
                "build".into(),
                "scan".into(),
                "sign".into(),
                "promote".into(),
            ],
            notes: vec![
                format!(
                    "POST /api/v1/enterprise/golden-pipeline/run applies CDI DataVolume+DataSource for {image_name}:{version}."
                ),
                "Scan/sign (cosign) remain operator-owned; promote is the stable DataSource alias.".into(),
            ],
        })
    }
}

/// Result of executing the build+promote stages against the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenPipelineRun {
    pub image_name: String,
    pub version: String,
    pub namespace: String,
    pub versioned_name: String,
    pub data_volume_applied: bool,
    pub data_source_applied: bool,
    pub stages_completed: Vec<String>,
    pub notes: Vec<String>,
}

impl GoldenPipelineRun {
    /// Build golden-image manifests and apply them via the KubeVirt/CDI client.
    pub async fn try_run(
        image_name: impl Into<String>,
        version: impl Into<String>,
        namespace: impl Into<String>,
        source: impl Into<String>,
        size: impl Into<String>,
        client: &crate::kube::KubeClient,
    ) -> Result<Self, String> {
        require("ZORVIA_FEATURE_GOLDEN_PIPELINE")?;
        let image_name = image_name.into();
        let version = version.into();
        let namespace = namespace.into();
        let source = source.into();
        let size = size.into();
        let source_type = if source.starts_with("http://") || source.starts_with("https://") {
            crate::golden_images::ImageSourceType::Http
        } else {
            crate::golden_images::ImageSourceType::Registry
        };
        let source = if source_type == crate::golden_images::ImageSourceType::Registry
            && !source.starts_with("docker://")
        {
            format!("docker://{source}")
        } else {
            source
        };
        let spec = crate::golden_images::GoldenImageSpec {
            name: image_name.clone(),
            version: version.clone(),
            namespace: namespace.clone(),
            source_type,
            source,
            size: if size.trim().is_empty() {
                "20Gi".into()
            } else {
                size
            },
            storage_class: None,
            checksum: None,
        };
        let bundle = crate::golden_images::GoldenImageBundle::build(spec)
            .map_err(|e| format!("bundle build failed: {e}"))?;
        client
            .apply_data_volume(&namespace, &bundle.data_volume)
            .await
            .map_err(|e| format!("DataVolume apply failed: {e}"))?;
        client
            .apply_data_source(&namespace, &bundle.data_source)
            .await
            .map_err(|e| format!("DataSource apply failed: {e}"))?;
        Ok(Self {
            image_name,
            version,
            namespace,
            versioned_name: bundle.versioned_name,
            data_volume_applied: true,
            data_source_applied: true,
            stages_completed: vec!["build".into(), "promote".into()],
            notes: vec![
                "build+promote applied via CDI. scan/sign skipped (no cosign hook in this release)."
                    .into(),
            ],
        })
    }
}

/// Cross-cluster VM mobility / DR plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossClusterDrPlan {
    pub source_cluster: String,
    pub target_cluster: String,
    pub vm_names: Vec<String>,
    pub rpo_seconds: u64,
    pub notes: Vec<String>,
}

impl CrossClusterDrPlan {
    pub fn try_plan(
        source_cluster: impl Into<String>,
        target_cluster: impl Into<String>,
        vm_names: Vec<String>,
        rpo_seconds: u64,
    ) -> Result<Self, String> {
        require("ZORVIA_FEATURE_CROSS_CLUSTER_DR")?;
        if vm_names.is_empty() {
            return Err("vm_names must not be empty".into());
        }
        Ok(Self {
            source_cluster: source_cluster.into(),
            target_cluster: target_cluster.into(),
            vm_names,
            rpo_seconds,
            notes: vec![
                "Plan only — no cross-cluster data path is executed.".into(),
                "Pair with ZORVIA_FEATURE_S3_BACKUP for offsite snapshot copies.".into(),
            ],
        })
    }
}

/// GPU / SR-IOV / NUMA placement suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSriovNumaPlan {
    pub vm_name: String,
    pub gpu_count: u32,
    pub sriov_networks: Vec<String>,
    pub numa_passthrough: bool,
    pub suggested_patches: serde_json::Value,
    pub notes: Vec<String>,
}

impl GpuSriovNumaPlan {
    pub fn try_plan(
        vm_name: impl Into<String>,
        gpu_count: u32,
        sriov_networks: Vec<String>,
        numa_passthrough: bool,
    ) -> Result<Self, String> {
        require("ZORVIA_FEATURE_GPU_NUMA")?;
        let vm_name = vm_name.into();
        let suggested_patches = serde_json::json!({
            "domain": {
                "devices": {
                    "hostDevices": if gpu_count > 0 {
                        serde_json::json!([{ "name": "gpu", "deviceName": "nvidia.com/gpu", "count": gpu_count }])
                    } else {
                        serde_json::json!([])
                    },
                    "interfaces": sriov_networks.iter().map(|n| serde_json::json!({
                        "name": n,
                        "sriov": {},
                    })).collect::<Vec<_>>(),
                },
                "cpu": { "dedicatedCpuPlacement": numa_passthrough, "numa": { "guestMappingPassthrough": {} } },
            }
        });
        Ok(Self {
            vm_name,
            gpu_count,
            sriov_networks,
            numa_passthrough,
            suggested_patches,
            notes: vec![
                "Plan only — patches are not applied to the VM CR.".into(),
                "Requires device-plugin / SR-IOV CNI on target nodes.".into(),
            ],
        })
    }
}

/// Fleet multi-cluster inventory snapshot (empty until kubeconfigs are wired).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetInventory {
    pub clusters: Vec<serde_json::Value>,
    pub notes: Vec<String>,
}

impl FleetInventory {
    pub fn try_snapshot() -> Result<Self, String> {
        require("ZORVIA_FEATURE_FLEET")?;
        Ok(Self {
            clusters: Vec::new(),
            notes: vec![
                "Fleet inventory is empty until multi-kubeconfig discovery is configured.".into(),
                "See src/multi_cluster for the in-memory model.".into(),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_block_by_default() {
        if crate::features::experimental_enabled() {
            return;
        }
        assert!(S3ImmutableBackupPlan::try_plan("vm", "snap", "b", "us-east-1", "", 30).is_err());
        assert!(TransivaPlan::try_plan("vm", "https://vc", "default").is_err());
        assert!(GoldenPipelinePlan::try_plan("img", "1.0", "default").is_err());
        assert!(CrossClusterDrPlan::try_plan("a", "b", vec!["vm".into()], 300).is_err());
        assert!(GpuSriovNumaPlan::try_plan("vm", 1, vec!["net1".into()], true).is_err());
        assert!(FleetInventory::try_snapshot().is_err());
    }
}
