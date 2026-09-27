// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! DTOs mirroring `atlas-api-types` (`../atlas/crates/atlas-api-types/src/lib.rs`).
//! Atlas's gateway returns typed JSON for backends/storage-classes but raw
//! `serde_json::Value` for clusters/pools/volumes/ceph-status/ceph-df/create-volume
//! (see `atlas-gateway/src/routes/{inventory,volumes}.rs`) — this module matches
//! that contract exactly rather than guessing an untyped route into a typed one.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    Ceph,
    Nfs,
    Zfs,
    San,
    CloudBlock,
    Kubernetes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendMode {
    ManagedRook,
    External,
    ReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Ok,
    Warn,
    Critical,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VolumeKind {
    #[default]
    Block,
    Filesystem,
    Object,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Capabilities {
    pub block: bool,
    pub file: bool,
    pub object: bool,
    pub snapshots: bool,
    pub clone: bool,
    pub expansion: bool,
    pub replication: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBackend {
    pub id: String,
    pub name: String,
    pub backend_type: BackendType,
    pub mode: BackendMode,
    pub status: String,
    #[serde(default)]
    pub capabilities: Capabilities,
    pub connection_ref: Option<String>,
    #[serde(default)]
    pub cordoned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageClassInfo {
    pub name: String,
    pub provisioner: String,
    pub reclaim_policy: Option<String>,
    pub volume_binding_mode: Option<String>,
    pub allow_volume_expansion: Option<bool>,
    #[serde(default)]
    pub is_ceph: bool,
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

/// Sibling-product ownership tag for a volume (PDF §5.3, §11 `product_bindings`
/// in Atlas). Populate this when creating a volume in the context of a VM so
/// it's traceable back to Zorvia in Atlas's own inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Owner {
    pub product: String,
    pub resource_type: String,
    pub resource_id: String,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "data_disk".into()
}

impl Owner {
    /// The shape Zorvia always uses: a volume owned by one of its own VMs.
    pub fn for_vm(vm_name: &str) -> Self {
        Self {
            product: "zorvia".into(),
            resource_type: "vm".into(),
            resource_id: vm_name.to_string(),
            role: default_role(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct K8sVolumeOpts {
    pub namespace: Option<String>,
    #[serde(default = "default_true")]
    pub create_pvc: bool,
    #[serde(default)]
    pub access_modes: Vec<String>,
    pub volume_mode: Option<String>,
    pub storage_class: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVolumeRequest {
    pub tenant_id: String,
    pub name: String,
    pub size_bytes: i64,
    #[serde(default)]
    pub kind: VolumeKind,
    pub policy: Option<String>,
    pub pool: Option<String>,
    #[serde(default)]
    pub owner: Option<Owner>,
    #[serde(default)]
    pub kubernetes: Option<K8sVolumeOpts>,
}
