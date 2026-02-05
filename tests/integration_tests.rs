use zorvia::config::{DiskSource, NetworkType, VMConfigBuilder};
use zorvia::kube::vm_config_to_kubevirt;
use zorvia::output::{to_json, to_yaml};
use zorvia::templates::TEMPLATES;

#[test]
fn test_template_to_kubevirt_conversion() {
    let template = TEMPLATES.get("ubuntu").expect("ubuntu template exists");
    let vm = vm_config_to_kubevirt(&template).unwrap();

    assert_eq!(vm.metadata.name, Some("ubuntu-vm".to_string()));
    assert_eq!(vm.spec.running, Some(false));
    assert!(vm.spec.template.spec.volumes.is_some());

    let volumes = vm.spec.template.spec.volumes.as_ref().unwrap();
    assert!(volumes.len() >= 2); // rootdisk + datadisk + cloudinit
}

#[test]
fn test_complex_vm_build() {
    let config = VMConfigBuilder::new("complex-vm")
        .namespace("production")
        .cpu(8, 2, 2)
        .cpu_model("host-passthrough")
        .memory("32Gi")
        .add_blank_disk("rootdisk", "100Gi", 1)
        .add_blank_disk("datadisk", "500Gi", 2)
        .add_container_disk("cdrom", "registry.example.com/iso:latest", 3)
        .add_pod_network("management")
        .label("app", "database")
        .label("tier", "backend")
        .label("environment", "production")
        .cloud_init(
            r#"#cloud-config
users:
  - name: admin
    sudo: ALL=(ALL) NOPASSWD:ALL
"#,
        )
        .build();

    // Validate
    zorvia::config::validate_vm_config(&config).unwrap();

    // Convert to KubeVirt
    let vm = vm_config_to_kubevirt(&config).unwrap();

    assert_eq!(
        vm.spec.template.spec.domain.cpu.as_ref().unwrap().cores,
        Some(8)
    );
    assert_eq!(
        vm.spec.template.spec.domain.cpu.as_ref().unwrap().sockets,
        Some(2)
    );
    assert_eq!(
        vm.spec.template.spec.domain.cpu.as_ref().unwrap().threads,
        Some(2)
    );
    assert_eq!(
        vm.spec
            .template
            .spec
            .domain
            .cpu
            .as_ref()
            .unwrap()
            .model
            .as_deref(),
        Some("host-passthrough")
    );

    let volumes = vm.spec.template.spec.volumes.as_ref().unwrap();
    assert_eq!(volumes.len(), 4); // 3 disks + cloudinit

    // Check cloud-init volume exists
    let cloudinit_vol = volumes.iter().find(|v| v.name == "cloudinitdisk");
    assert!(cloudinit_vol.is_some());
    assert!(cloudinit_vol
        .unwrap()
        .cloud_init_no_cloud
        .as_ref()
        .unwrap()
        .user_data
        .is_some());
}

#[test]
fn test_yaml_serialization_roundtrip() {
    let config = VMConfigBuilder::new("test-vm")
        .namespace("default")
        .cpu(4, 1, 1)
        .memory("8Gi")
        .add_blank_disk("rootdisk", "40Gi", 1)
        .add_pod_network("default")
        .build();

    // Serialize to YAML
    let yaml = to_yaml(&config).unwrap();

    // Deserialize back
    let deserialized: zorvia::config::VMConfig = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(config.name, deserialized.name);
    assert_eq!(config.namespace, deserialized.namespace);
    assert_eq!(config.cpu.cores, deserialized.cpu.cores);
    assert_eq!(config.memory.size, deserialized.memory.size);
    assert_eq!(config.disks.len(), deserialized.disks.len());
}

#[test]
fn test_json_serialization_roundtrip() {
    let config = VMConfigBuilder::new("test-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_blank_disk("rootdisk", "20Gi", 1)
        .add_pod_network("default")
        .build();

    // Serialize to JSON
    let json = to_json(&config).unwrap();

    // Deserialize back
    let deserialized: zorvia::config::VMConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.name, deserialized.name);
    assert_eq!(config.namespace, deserialized.namespace);
}

#[test]
fn test_all_disk_types_conversion() {
    let mut config = VMConfigBuilder::new("all-disks")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .build();

    // Add all disk types
    config.disks.push(zorvia::config::DiskConfig {
        name: "blank-disk".to_string(),
        size: "10Gi".to_string(),
        storage_class: Some("fast-ssd".to_string()),
        boot_order: 1,
        source: DiskSource::Blank,
    });

    config.disks.push(zorvia::config::DiskConfig {
        name: "pvc-disk".to_string(),
        size: "20Gi".to_string(),
        storage_class: None,
        boot_order: 2,
        source: DiskSource::PVC {
            name: "existing-pvc".to_string(),
        },
    });

    config.disks.push(zorvia::config::DiskConfig {
        name: "container-disk".to_string(),
        size: "0".to_string(),
        storage_class: None,
        boot_order: 3,
        source: DiskSource::ContainerDisk {
            image: "registry.io/os:latest".to_string(),
        },
    });

    config.disks.push(zorvia::config::DiskConfig {
        name: "dv-disk".to_string(),
        size: "50Gi".to_string(),
        storage_class: None,
        boot_order: 4,
        source: DiskSource::DataVolume {
            name: "my-datavolume".to_string(),
        },
    });

    config.interfaces.push(zorvia::config::InterfaceConfig {
        name: "default".to_string(),
        network: "default".to_string(),
        model: "virtio".to_string(),
        network_type: NetworkType::Pod,
    });

    let vm = vm_config_to_kubevirt(&config).unwrap();
    let volumes = vm.spec.template.spec.volumes.as_ref().unwrap();

    assert_eq!(volumes.len(), 4);

    // Verify each disk type was converted
    let blank = volumes.iter().find(|v| v.name == "blank-disk");
    assert!(blank.is_some());
    assert!(blank.unwrap().empty_disk.is_some());

    let pvc = volumes.iter().find(|v| v.name == "pvc-disk");
    assert!(pvc.is_some());
    assert!(pvc.unwrap().persistent_volume_claim.is_some());

    let container = volumes.iter().find(|v| v.name == "container-disk");
    assert!(container.is_some());
    assert!(container.unwrap().container_disk.is_some());

    let dv = volumes.iter().find(|v| v.name == "dv-disk");
    assert!(dv.is_some());
    assert!(dv.unwrap().data_volume.is_some());
}

#[test]
fn test_network_types_conversion() {
    let mut config = VMConfigBuilder::new("network-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_blank_disk("rootdisk", "20Gi", 1)
        .build();

    config.interfaces.push(zorvia::config::InterfaceConfig {
        name: "pod-net".to_string(),
        network: "default".to_string(),
        model: "virtio".to_string(),
        network_type: NetworkType::Pod,
    });

    config.interfaces.push(zorvia::config::InterfaceConfig {
        name: "bridge-net".to_string(),
        network: "br0".to_string(),
        model: "e1000".to_string(),
        network_type: NetworkType::Bridge,
    });

    config.interfaces.push(zorvia::config::InterfaceConfig {
        name: "multus-net".to_string(),
        network: "data-network".to_string(),
        model: "virtio".to_string(),
        network_type: NetworkType::Multus {
            name: "nad-data-network".to_string(),
        },
    });

    let vm = vm_config_to_kubevirt(&config).unwrap();
    let interfaces = vm.spec.template.spec.domain.devices.as_ref().unwrap().interfaces.as_ref().unwrap();
    let networks = vm.spec.template.spec.networks.as_ref().unwrap();

    assert_eq!(interfaces.len(), 3);
    assert_eq!(networks.len(), 3);

    // Check Multus network was converted
    let multus_net = networks.iter().find(|n| n.name == "multus-net");
    assert!(multus_net.is_some());
    assert!(multus_net.unwrap().multus.is_some());
    assert_eq!(
        multus_net.unwrap().multus.as_ref().unwrap().network_name,
        "nad-data-network"
    );
}
