use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// VirtualMachine custom resource for KubeVirt
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "kubevirt.io",
    version = "v1",
    kind = "VirtualMachine",
    plural = "virtualmachines",
    shortname = "vm",
    shortname = "vms",
    namespaced
)]
#[kube(status = "VirtualMachineStatus")]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineSpec {
    pub running: Option<bool>,
    pub run_strategy: Option<String>,
    pub template: VirtualMachineInstanceTemplateSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineInstanceTemplateSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "metadata_schema")]
    pub metadata: Option<ObjectMeta>,
    pub spec: VirtualMachineInstanceSpec,
}

fn metadata_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    schemars::schema::Schema::Bool(true)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineInstanceSpec {
    pub domain: DomainSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<Volume>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<Vec<Network>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub termination_grace_period_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DomainSpec {
    pub resources: ResourceRequirements,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CPU>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<Memory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devices: Option<Devices>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRequirements {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CPU {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sockets: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Devices {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disks: Option<Vec<Disk>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<Vec<Interface>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<DiskTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_order: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiskTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masquerade: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge: Option<BTreeMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_disk: Option<ContainerDiskSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent_volume_claim: Option<PersistentVolumeClaimVolumeSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_volume: Option<DataVolumeSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_init_no_cloud: Option<CloudInitNoCloudSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_disk: Option<EmptyDiskSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDiskSource {
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_pull_policy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersistentVolumeClaimVolumeSource {
    pub claim_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DataVolumeSource {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudInitNoCloudSource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDiskSource {
    pub capacity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multus: Option<MultusNetwork>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MultusNetwork {
    pub network_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Condition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_able_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_probe_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_transition_time: Option<String>,
}

/// VirtualMachineInstance - represents a running VM instance in KubeVirt
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "kubevirt.io",
    version = "v1",
    kind = "VirtualMachineInstance",
    plural = "virtualmachineinstances",
    shortname = "vmi",
    shortname = "vmis",
    namespaced
)]
#[kube(status = "VirtualMachineInstanceStatus")]
#[serde(rename_all = "camelCase")]
pub struct VmiSpec {
    // VMI spec mirrors the template spec - we mainly care about status
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineInstanceStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
    #[serde(default)]
    pub interfaces: Vec<VmiInterface>,
    #[serde(
        rename = "guestOSInfo",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub guest_os_info: Option<GuestOsInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VmiInterface {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub ip_addresses: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuestOsInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_release: Option<String>,
}

/// VirtualMachineInstanceMigration CRD for KubeVirt
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "kubevirt.io",
    version = "v1",
    kind = "VirtualMachineInstanceMigration",
    plural = "virtualmachineinstancemigrations",
    shortname = "vmim",
    namespaced
)]
#[kube(status = "VirtualMachineInstanceMigrationStatus")]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineInstanceMigrationSpec {
    /// Name of the VMI to migrate
    pub vmi_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachineInstanceMigrationStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_state: Option<MigrationStateInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStateInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_node_address: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vmi_status_deserialization() {
        let json = serde_json::json!({
            "phase": "Running",
            "nodeName": "worker-1",
            "interfaces": [
                {
                    "name": "default",
                    "ipAddress": "10.244.1.5",
                    "ipAddresses": ["10.244.1.5", "fd00::5"],
                    "mac": "52:54:00:12:34:56",
                    "interfaceName": "eth0"
                }
            ],
            "guestOSInfo": {
                "name": "Fedora Linux",
                "id": "fedora",
                "version": "39",
                "kernelRelease": "6.5.0-0.rc7"
            }
        });

        let status: VirtualMachineInstanceStatus = serde_json::from_value(json).unwrap();
        assert_eq!(status.phase.as_deref(), Some("Running"));
        assert_eq!(status.node_name.as_deref(), Some("worker-1"));
        assert_eq!(status.interfaces.len(), 1);
        assert_eq!(
            status.interfaces[0].ip_address.as_deref(),
            Some("10.244.1.5")
        );
        assert_eq!(status.interfaces[0].ip_addresses.len(), 2);
        assert_eq!(
            status.interfaces[0].mac.as_deref(),
            Some("52:54:00:12:34:56")
        );
        assert_eq!(
            status.interfaces[0].interface_name.as_deref(),
            Some("eth0")
        );
        let guest_os = status.guest_os_info.as_ref().unwrap();
        assert_eq!(guest_os.name.as_deref(), Some("Fedora Linux"));
        assert_eq!(guest_os.id.as_deref(), Some("fedora"));
        assert_eq!(guest_os.version.as_deref(), Some("39"));
    }

    #[test]
    fn test_vmi_status_deserialization_minimal() {
        let json = serde_json::json!({
            "phase": "Scheduling",
            "interfaces": []
        });

        let status: VirtualMachineInstanceStatus = serde_json::from_value(json).unwrap();
        assert_eq!(status.phase.as_deref(), Some("Scheduling"));
        assert!(status.node_name.is_none());
        assert!(status.interfaces.is_empty());
        assert!(status.guest_os_info.is_none());
    }

    #[test]
    fn test_vmi_status_serialization_roundtrip() {
        let status = VirtualMachineInstanceStatus {
            phase: Some("Running".to_string()),
            node_name: Some("node-1".to_string()),
            interfaces: vec![VmiInterface {
                name: Some("default".to_string()),
                ip_address: Some("192.168.1.10".to_string()),
                ip_addresses: vec![
                    "192.168.1.10".to_string(),
                    "fd00::a".to_string(),
                ],
                mac: Some("aa:bb:cc:dd:ee:ff".to_string()),
                interface_name: Some("eth0".to_string()),
            }],
            guest_os_info: Some(GuestOsInfo {
                name: Some("Ubuntu".to_string()),
                id: Some("ubuntu".to_string()),
                version: Some("22.04".to_string()),
                kernel_release: Some("5.15.0".to_string()),
            }),
        };

        let json = serde_json::to_value(&status).unwrap();
        let deserialized: VirtualMachineInstanceStatus =
            serde_json::from_value(json).unwrap();

        assert_eq!(deserialized.phase, status.phase);
        assert_eq!(deserialized.node_name, status.node_name);
        assert_eq!(deserialized.interfaces.len(), 1);
        assert_eq!(
            deserialized.interfaces[0].ip_address,
            status.interfaces[0].ip_address
        );
    }

    #[test]
    fn test_vmi_interface_empty_fields() {
        let json = serde_json::json!({});

        let iface: VmiInterface = serde_json::from_value(json).unwrap();
        assert!(iface.name.is_none());
        assert!(iface.ip_address.is_none());
        assert!(iface.ip_addresses.is_empty());
        assert!(iface.mac.is_none());
        assert!(iface.interface_name.is_none());
    }

    #[test]
    fn test_vmi_interface_multiple_ips() {
        let json = serde_json::json!({
            "name": "net1",
            "ipAddress": "10.0.0.1",
            "ipAddresses": ["10.0.0.1", "10.0.0.2", "fd00::1"],
            "mac": "52:54:00:ab:cd:ef",
            "interfaceName": "net1"
        });

        let iface: VmiInterface = serde_json::from_value(json).unwrap();
        assert_eq!(iface.ip_addresses.len(), 3);
        assert_eq!(iface.ip_addresses[0], "10.0.0.1");
        assert_eq!(iface.ip_addresses[2], "fd00::1");
    }

    #[test]
    fn test_guest_os_info_partial() {
        let json = serde_json::json!({
            "name": "CentOS Stream",
            "id": "centos"
        });

        let info: GuestOsInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.name.as_deref(), Some("CentOS Stream"));
        assert_eq!(info.id.as_deref(), Some("centos"));
        assert!(info.version.is_none());
        assert!(info.kernel_release.is_none());
    }

    #[test]
    fn test_vmi_status_skips_none_on_serialize() {
        let status = VirtualMachineInstanceStatus {
            phase: Some("Running".to_string()),
            node_name: None,
            interfaces: vec![],
            guest_os_info: None,
        };

        let json = serde_json::to_value(&status).unwrap();
        assert!(json.get("phase").is_some());
        assert!(json.get("nodeName").is_none());
        assert!(json.get("guestOSInfo").is_none());
    }

    /// Helper to extract the first IP from a VirtualMachineInstanceStatus,
    /// mirroring the logic in KubeClient::get_vm_ip.
    fn extract_first_ip(status: &VirtualMachineInstanceStatus) -> Option<String> {
        for iface in &status.interfaces {
            if let Some(ip) = &iface.ip_address {
                if !ip.is_empty() {
                    return Some(ip.clone());
                }
            }
        }
        None
    }

    #[test]
    fn test_extract_ip_from_interfaces() {
        let status = VirtualMachineInstanceStatus {
            phase: Some("Running".to_string()),
            node_name: None,
            interfaces: vec![
                VmiInterface {
                    name: Some("default".to_string()),
                    ip_address: Some("10.244.1.5".to_string()),
                    ip_addresses: vec!["10.244.1.5".to_string()],
                    mac: None,
                    interface_name: None,
                },
                VmiInterface {
                    name: Some("net1".to_string()),
                    ip_address: Some("192.168.1.100".to_string()),
                    ip_addresses: vec!["192.168.1.100".to_string()],
                    mac: None,
                    interface_name: None,
                },
            ],
            guest_os_info: None,
        };

        // Should return the first interface's IP
        assert_eq!(extract_first_ip(&status), Some("10.244.1.5".to_string()));
    }

    #[test]
    fn test_extract_ip_skips_empty() {
        let status = VirtualMachineInstanceStatus {
            phase: Some("Running".to_string()),
            node_name: None,
            interfaces: vec![
                VmiInterface {
                    name: Some("default".to_string()),
                    ip_address: Some(String::new()),
                    ip_addresses: vec![],
                    mac: None,
                    interface_name: None,
                },
                VmiInterface {
                    name: Some("net1".to_string()),
                    ip_address: Some("10.0.0.1".to_string()),
                    ip_addresses: vec!["10.0.0.1".to_string()],
                    mac: None,
                    interface_name: None,
                },
            ],
            guest_os_info: None,
        };

        // Should skip empty string and return the second IP
        assert_eq!(extract_first_ip(&status), Some("10.0.0.1".to_string()));
    }

    #[test]
    fn test_extract_ip_no_interfaces() {
        let status = VirtualMachineInstanceStatus {
            phase: Some("Scheduling".to_string()),
            node_name: None,
            interfaces: vec![],
            guest_os_info: None,
        };

        assert_eq!(extract_first_ip(&status), None);
    }

    #[test]
    fn test_extract_ip_none_ip_address() {
        let status = VirtualMachineInstanceStatus {
            phase: Some("Running".to_string()),
            node_name: None,
            interfaces: vec![VmiInterface {
                name: Some("default".to_string()),
                ip_address: None,
                ip_addresses: vec![],
                mac: None,
                interface_name: None,
            }],
            guest_os_info: None,
        };

        assert_eq!(extract_first_ip(&status), None);
    }
}
