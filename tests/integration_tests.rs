use zorvia::config::{DiskSource, NetworkType, VMConfig, VMConfigBuilder, validate_vm_config};
use zorvia::kube::vm_config_to_kubevirt;
use zorvia::output::{to_json, to_yaml};
use zorvia::templates::TEMPLATES;
use zorvia::profiles::PROFILES;
use zorvia::blueprints::BLUEPRINTS;
use zorvia::storage::{parse_size_to_bytes, format_bytes, PvcSpec, AccessMode};

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

// ========== TEMPLATE TESTS ==========

#[test]
fn test_all_templates_valid() {
    let template_names = TEMPLATES.list();
    assert!(template_names.len() >= 40, "Expected at least 40 templates, got {}", template_names.len());

    for name in &template_names {
        let config = TEMPLATES.get(name);
        assert!(config.is_some(), "Template '{}' should exist", name);

        let config = config.unwrap();
        assert!(!config.name.is_empty(), "Template '{}' should have a name", name);
        assert!(config.cpu.cores >= 1, "Template '{}' should have at least 1 CPU core", name);
        assert!(!config.memory.size.is_empty(), "Template '{}' should have memory size", name);
    }
}

#[test]
fn test_all_templates_convert_to_kubevirt() {
    let template_names = TEMPLATES.list();

    for name in &template_names {
        let config = TEMPLATES.get(name).unwrap();
        let result = vm_config_to_kubevirt(&config);
        assert!(result.is_ok(), "Template '{}' should convert to KubeVirt: {:?}", name, result.err());
    }
}

#[test]
fn test_template_families() {
    let families = ["ubuntu", "fedora", "centos", "debian", "rhel", "windows"];
    for family in families {
        let config = TEMPLATES.get(family);
        assert!(config.is_some(), "Template family '{}' should have a default template", family);
    }
}

// ========== PROFILE TESTS ==========

#[test]
fn test_builtin_profiles_exist() {
    let manager = PROFILES.read().expect("profiles lock");
    let profiles = manager.list();

    let expected = ["minimal", "dev", "test", "web", "prod", "database", "microservice", "high-perf"];
    for name in expected {
        assert!(manager.exists(name), "Built-in profile '{}' should exist", name);
        assert!(manager.is_builtin(name), "Profile '{}' should be built-in", name);

        let profile = manager.get(name);
        assert!(profile.is_some(), "Profile '{}' should be retrievable", name);
        assert!(profile.unwrap().cpu_cores >= 1, "Profile '{}' should have at least 1 CPU", name);
    }

    assert!(profiles.len() >= 8, "Expected at least 8 profiles, got {}", profiles.len());
}

#[test]
fn test_profile_resource_ordering() {
    let manager = PROFILES.read().expect("profiles lock");

    let minimal = manager.get("minimal").unwrap();
    let dev = manager.get("dev").unwrap();
    let prod = manager.get("prod").unwrap();
    let high_perf = manager.get("high-perf").unwrap();

    // Resources should scale up
    assert!(minimal.cpu_cores <= dev.cpu_cores);
    assert!(dev.cpu_cores <= prod.cpu_cores);
    assert!(prod.cpu_cores <= high_perf.cpu_cores);
}

// ========== BLUEPRINT TESTS ==========

#[test]
fn test_builtin_blueprints_exist() {
    let manager = BLUEPRINTS.read().expect("blueprints lock");
    let blueprints = manager.list();

    let expected = ["lamp", "k8s-cluster", "3tier", "cicd", "dev-stack"];
    for name in expected {
        assert!(manager.exists(name), "Built-in blueprint '{}' should exist", name);
        assert!(manager.is_builtin(name), "Blueprint '{}' should be built-in", name);

        let bp = manager.get(name);
        assert!(bp.is_some(), "Blueprint '{}' should be retrievable", name);
        assert!(!bp.unwrap().vms.is_empty(), "Blueprint '{}' should have VMs", name);
    }

    assert!(blueprints.len() >= 5, "Expected at least 5 blueprints, got {}", blueprints.len());
}

#[test]
fn test_blueprint_vm_templates_valid() {
    let manager = BLUEPRINTS.read().expect("blueprints lock");

    for bp in manager.list() {
        for vm_spec in &bp.vms {
            let template = TEMPLATES.get(&vm_spec.template);
            assert!(template.is_some(),
                "Blueprint '{}' VM '{}' references template '{}' which doesn't exist",
                bp.name, vm_spec.name, vm_spec.template);
        }
    }
}

// ========== VALIDATION TESTS ==========

#[test]
fn test_example_files_valid_yaml() {
    let example_files = [
        "examples/basic-vm.yaml",
        "examples/ubuntu-cloud-init.yaml",
    ];

    for file in example_files {
        let content = std::fs::read_to_string(file);
        assert!(content.is_ok(), "Example file '{}' should be readable", file);

        let config: Result<VMConfig, _> = serde_yaml::from_str(&content.unwrap());
        assert!(config.is_ok(), "Example file '{}' should parse as VMConfig: {:?}", file, config.err());
    }
}

#[test]
fn test_validation_catches_invalid_name() {
    let config = VMConfigBuilder::new("INVALID_NAME!")
        .namespace("default")
        .cpu(1, 1, 1)
        .memory("1Gi")
        .add_blank_disk("root", "10Gi", 1)
        .add_pod_network("default")
        .build();

    let result = validate_vm_config(&config);
    assert!(result.is_err(), "Should reject invalid VM name");
}

#[test]
fn test_validation_catches_zero_cpu() {
    let config = VMConfigBuilder::new("test-vm")
        .namespace("default")
        .cpu(0, 1, 1)
        .memory("1Gi")
        .add_blank_disk("root", "10Gi", 1)
        .add_pod_network("default")
        .build();

    let result = validate_vm_config(&config);
    assert!(result.is_err(), "Should reject 0 CPU cores");
}

#[test]
fn test_validation_catches_invalid_memory() {
    let config = VMConfigBuilder::new("test-vm")
        .namespace("default")
        .cpu(1, 1, 1)
        .memory("invalid")
        .add_blank_disk("root", "10Gi", 1)
        .add_pod_network("default")
        .build();

    let result = validate_vm_config(&config);
    assert!(result.is_err(), "Should reject invalid memory format");
}

// ========== STORAGE UTILITY TESTS ==========

#[test]
fn test_storage_size_conversions() {
    assert_eq!(parse_size_to_bytes("100Gi"), Some(100 * 1024 * 1024 * 1024));
    assert_eq!(parse_size_to_bytes("512Mi"), Some(512 * 1024 * 1024));

    let bytes = parse_size_to_bytes("10Gi").unwrap();
    let formatted = format_bytes(bytes);
    assert_eq!(formatted, "10.0Gi");
}

#[test]
fn test_pvc_spec_for_vm_disk() {
    let pvc = PvcSpec::new("my-vm-rootdisk", "100Gi")
        .with_namespace("production")
        .with_storage_class("ceph-rbd")
        .with_access_mode(AccessMode::ReadWriteOnce)
        .with_label("app", "database")
        .with_label("managed-by", "zorvia");

    assert_eq!(pvc.name, "my-vm-rootdisk");
    assert_eq!(pvc.size, "100Gi");
    assert_eq!(pvc.namespace, "production");
    assert_eq!(pvc.storage_class.as_deref(), Some("ceph-rbd"));
    assert_eq!(pvc.labels.len(), 2);
}

// ========== END-TO-END WORKFLOW TESTS ==========

#[test]
fn test_template_to_config_to_kubevirt_workflow() {
    // Simulate: user picks a template, customizes it, converts to KubeVirt
    let mut config = TEMPLATES.get("ubuntu").unwrap();
    config.name = "my-web-server".to_string();
    config.namespace = "production".to_string();
    config.cpu.cores = 4;
    config.memory.size = "16Gi".to_string();
    config.labels.insert("app".to_string(), "web".to_string());

    // Validate
    validate_vm_config(&config).unwrap();

    // Convert to KubeVirt
    let vm = vm_config_to_kubevirt(&config).unwrap();
    assert_eq!(vm.metadata.name, Some("my-web-server".to_string()));
    assert_eq!(vm.metadata.namespace, Some("production".to_string()));
    assert_eq!(vm.spec.template.spec.domain.cpu.as_ref().unwrap().cores, Some(4));

    // Serialize to YAML (what kubectl would apply)
    let yaml = to_yaml(&vm).unwrap();
    assert!(yaml.contains("my-web-server"));
    assert!(yaml.contains("production"));
}

#[test]
fn test_profile_applied_to_template() {
    let manager = PROFILES.read().expect("profiles lock");
    let profile = manager.get("database").unwrap();

    let mut config = TEMPLATES.get("ubuntu").unwrap();
    config.name = "db-server".to_string();
    config.cpu.cores = profile.cpu_cores;
    config.cpu.sockets = profile.cpu_sockets;
    config.memory.size = profile.memory.clone();

    validate_vm_config(&config).unwrap();

    assert_eq!(config.cpu.cores, profile.cpu_cores);
    assert_eq!(config.memory.size, profile.memory);
}
