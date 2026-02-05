use crate::config::{DiskSource, NetworkType, VMConfig};
use crate::kube::types::*;
use anyhow::Result;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use std::collections::BTreeMap;

/// Convert VMConfig to KubeVirt VirtualMachine CRD
pub fn vm_config_to_kubevirt(config: &VMConfig) -> Result<VirtualMachine> {
    let mut labels: BTreeMap<String, String> = config.labels.clone().into_iter().collect();
    labels.insert("kubevirt.io/vm".to_string(), config.name.clone());

    let annotations: BTreeMap<String, String> = config.annotations.clone().into_iter().collect();

    let metadata = ObjectMeta {
        name: Some(config.name.clone()),
        namespace: Some(config.namespace.clone()),
        labels: Some(labels),
        annotations: if annotations.is_empty() {
            None
        } else {
            Some(annotations)
        },
        ..Default::default()
    };

    // Build volumes
    let mut volumes = Vec::new();

    for disk in &config.disks {
        let volume = match &disk.source {
            DiskSource::Blank => Volume {
                name: disk.name.clone(),
                empty_disk: Some(EmptyDiskSource {
                    capacity: disk.size.clone(),
                }),
                container_disk: None,
                persistent_volume_claim: None,
                data_volume: None,
                cloud_init_no_cloud: None,
            },
            DiskSource::PVC { name } => Volume {
                name: disk.name.clone(),
                persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                    claim_name: name.clone(),
                }),
                empty_disk: None,
                container_disk: None,
                data_volume: None,
                cloud_init_no_cloud: None,
            },
            DiskSource::ContainerDisk { image } => Volume {
                name: disk.name.clone(),
                container_disk: Some(ContainerDiskSource {
                    image: image.clone(),
                    image_pull_policy: Some("IfNotPresent".to_string()),
                }),
                empty_disk: None,
                persistent_volume_claim: None,
                data_volume: None,
                cloud_init_no_cloud: None,
            },
            DiskSource::DataVolume { name } => Volume {
                name: disk.name.clone(),
                data_volume: Some(DataVolumeSource { name: name.clone() }),
                empty_disk: None,
                container_disk: None,
                persistent_volume_claim: None,
                cloud_init_no_cloud: None,
            },
        };
        volumes.push(volume);
    }

    // Add cloud-init volume if present
    if let Some(cloud_init) = &config.cloud_init {
        volumes.push(Volume {
            name: "cloudinitdisk".to_string(),
            cloud_init_no_cloud: Some(CloudInitNoCloudSource {
                user_data: Some(cloud_init.user_data.clone()),
                network_data: cloud_init.network_data.clone(),
            }),
            empty_disk: None,
            container_disk: None,
            persistent_volume_claim: None,
            data_volume: None,
        });
    }

    // Build disks
    let mut disks = Vec::new();
    for disk in &config.disks {
        disks.push(Disk {
            name: disk.name.clone(),
            disk: Some(DiskTarget {
                bus: Some("virtio".to_string()),
            }),
            boot_order: if disk.boot_order > 0 {
                Some(disk.boot_order)
            } else {
                None
            },
        });
    }

    // Add cloud-init disk if present
    if config.cloud_init.is_some() {
        disks.push(Disk {
            name: "cloudinitdisk".to_string(),
            disk: Some(DiskTarget {
                bus: Some("virtio".to_string()),
            }),
            boot_order: None,
        });
    }

    // Build interfaces
    let mut interfaces = Vec::new();
    for iface in &config.interfaces {
        let (masquerade, bridge) = match &iface.network_type {
            NetworkType::Pod => (Some(BTreeMap::new()), None),
            NetworkType::Bridge => (None, Some(BTreeMap::new())),
            NetworkType::Multus { .. } => (None, Some(BTreeMap::new())),
        };

        interfaces.push(Interface {
            name: iface.name.clone(),
            model: Some(iface.model.clone()),
            masquerade,
            bridge,
        });
    }

    // Build networks
    let mut networks = Vec::new();
    for iface in &config.interfaces {
        let network = match &iface.network_type {
            NetworkType::Pod => Network {
                name: iface.name.clone(),
                pod: Some(BTreeMap::new()),
                multus: None,
            },
            NetworkType::Bridge => Network {
                name: iface.name.clone(),
                pod: Some(BTreeMap::new()),
                multus: None,
            },
            NetworkType::Multus { name } => Network {
                name: iface.name.clone(),
                pod: None,
                multus: Some(MultusNetwork {
                    network_name: name.clone(),
                }),
            },
        };
        networks.push(network);
    }

    // Build resource requests
    let mut requests = BTreeMap::new();
    requests.insert("memory".to_string(), config.memory.size.clone());

    let spec = VirtualMachineSpec {
        running: Some(false), // VMs are created in stopped state
        run_strategy: None,
        template: VirtualMachineInstanceTemplateSpec {
            metadata: Some(ObjectMeta {
                labels: Some(config.labels.clone().into_iter().collect()),
                ..Default::default()
            }),
            spec: VirtualMachineInstanceSpec {
                domain: DomainSpec {
                    resources: ResourceRequirements {
                        requests: Some(requests),
                        limits: None,
                    },
                    cpu: Some(CPU {
                        cores: Some(config.cpu.cores),
                        sockets: Some(config.cpu.sockets),
                        threads: Some(config.cpu.threads),
                        model: config.cpu.model.clone(),
                    }),
                    memory: Some(Memory {
                        guest: Some(config.memory.size.clone()),
                    }),
                    devices: Some(Devices {
                        disks: Some(disks),
                        interfaces: Some(interfaces),
                    }),
                },
                volumes: Some(volumes),
                networks: Some(networks),
                termination_grace_period_seconds: Some(30),
            },
        },
    };

    Ok(VirtualMachine {
        metadata,
        spec,
        status: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::VMConfigBuilder;

    #[test]
    fn test_basic_conversion() {
        let config = VMConfigBuilder::new("test-vm")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .add_pod_network("default")
            .build();

        let vm = vm_config_to_kubevirt(&config).unwrap();

        assert_eq!(vm.metadata.name, Some("test-vm".to_string()));
        assert_eq!(vm.metadata.namespace, Some("default".to_string()));
        assert_eq!(vm.spec.running, Some(false));

        let vmi_spec = &vm.spec.template.spec;
        assert_eq!(vmi_spec.domain.cpu.as_ref().unwrap().cores, Some(2));
        assert_eq!(
            vmi_spec.domain.memory.as_ref().unwrap().guest,
            Some("4Gi".to_string())
        );
    }

    #[test]
    fn test_container_disk_conversion() {
        let config = VMConfigBuilder::new("fedora-vm")
            .namespace("default")
            .cpu(4, 1, 1)
            .memory("8Gi")
            .add_container_disk("rootdisk", "quay.io/containerdisks/fedora:39", 1)
            .add_pod_network("default")
            .build();

        let vm = vm_config_to_kubevirt(&config).unwrap();
        let volumes = vm.spec.template.spec.volumes.as_ref().unwrap();

        assert_eq!(volumes.len(), 1);
        assert!(volumes[0].container_disk.is_some());
        assert_eq!(
            volumes[0].container_disk.as_ref().unwrap().image,
            "quay.io/containerdisks/fedora:39"
        );
    }

    #[test]
    fn test_cloud_init_conversion() {
        let config = VMConfigBuilder::new("ubuntu-vm")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .add_pod_network("default")
            .cloud_init("#cloud-config\nuser: ubuntu")
            .build();

        let vm = vm_config_to_kubevirt(&config).unwrap();
        let volumes = vm.spec.template.spec.volumes.as_ref().unwrap();

        // Should have rootdisk + cloudinitdisk
        assert_eq!(volumes.len(), 2);

        let cloud_init_vol = volumes.iter().find(|v| v.name == "cloudinitdisk");
        assert!(cloud_init_vol.is_some());
        assert!(cloud_init_vol.unwrap().cloud_init_no_cloud.is_some());
    }
}
