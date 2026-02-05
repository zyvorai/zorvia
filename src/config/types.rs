use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main VM configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VMConfig {
    pub name: String,
    pub namespace: String,
    pub cpu: CPUConfig,
    pub memory: MemoryConfig,
    pub disks: Vec<DiskConfig>,
    pub interfaces: Vec<InterfaceConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_init: Option<CloudInitConfig>,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// CPU configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CPUConfig {
    pub cores: u32,
    #[serde(default = "default_sockets")]
    pub sockets: u32,
    #[serde(default = "default_threads")]
    pub threads: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for CPUConfig {
    fn default() -> Self {
        Self {
            cores: 1,
            sockets: 1,
            threads: 1,
            model: None,
        }
    }
}

fn default_sockets() -> u32 {
    1
}

fn default_threads() -> u32 {
    1
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub size: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            size: "2Gi".to_string(),
        }
    }
}

/// Disk configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskConfig {
    pub name: String,
    pub size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
    pub boot_order: u32,
    pub source: DiskSource,
}

/// Disk source types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DiskSource {
    Blank,
    #[serde(rename = "pvc")]
    PVC { name: String },
    ContainerDisk { image: String },
    DataVolume { name: String },
}

/// Network interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub network: String,
    #[serde(default = "default_interface_model")]
    pub model: String,
    pub network_type: NetworkType,
}

fn default_interface_model() -> String {
    "virtio".to_string()
}

/// Network type options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    Bridge,
    Multus { name: String },
    Pod,
}

impl Default for NetworkType {
    fn default() -> Self {
        NetworkType::Pod
    }
}

/// Cloud-init configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudInitConfig {
    pub user_data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_data: Option<String>,
}
