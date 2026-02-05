use super::types::*;
use anyhow::{anyhow, Result};

/// Validates a VM configuration
pub fn validate_vm_config(config: &VMConfig) -> Result<()> {
    validate_name(&config.name)?;
    validate_namespace(&config.namespace)?;
    validate_cpu(&config.cpu)?;
    validate_memory(&config.memory)?;
    validate_disks(&config.disks)?;
    validate_interfaces(&config.interfaces)?;

    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("VM name cannot be empty"));
    }

    if name.len() > 253 {
        return Err(anyhow!("VM name cannot exceed 253 characters"));
    }

    // Kubernetes name validation: lowercase alphanumeric, '-', '.'
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
    {
        return Err(anyhow!(
            "VM name must contain only lowercase alphanumeric characters, '-', or '.'"
        ));
    }

    Ok(())
}

fn validate_namespace(namespace: &str) -> Result<()> {
    if namespace.is_empty() {
        return Err(anyhow!("Namespace cannot be empty"));
    }

    Ok(())
}

fn validate_cpu(cpu: &CPUConfig) -> Result<()> {
    if cpu.cores == 0 {
        return Err(anyhow!("CPU cores must be greater than 0"));
    }

    if cpu.sockets == 0 {
        return Err(anyhow!("CPU sockets must be greater than 0"));
    }

    if cpu.threads == 0 {
        return Err(anyhow!("CPU threads must be greater than 0"));
    }

    // Reasonable upper limits
    let total_cpus = cpu.cores * cpu.sockets * cpu.threads;
    if total_cpus > 256 {
        return Err(anyhow!(
            "Total CPU count ({}) exceeds reasonable limit (256)",
            total_cpus
        ));
    }

    Ok(())
}

fn validate_memory(memory: &MemoryConfig) -> Result<()> {
    validate_memory_size(&memory.size)?;
    Ok(())
}

fn validate_memory_size(size: &str) -> Result<()> {
    let re = regex::Regex::new(r"^(\d+)(Mi|Gi|Ti|M|G|T)$").unwrap();

    if !re.is_match(size) {
        return Err(anyhow!(
            "Invalid memory size format: '{}'. Expected format: <number>(Mi|Gi|Ti|M|G|T)",
            size
        ));
    }

    Ok(())
}

fn validate_disks(disks: &[DiskConfig]) -> Result<()> {
    if disks.is_empty() {
        return Err(anyhow!("At least one disk must be configured"));
    }

    // Check for duplicate names
    let mut names = std::collections::HashSet::new();
    for disk in disks {
        if !names.insert(&disk.name) {
            return Err(anyhow!("Duplicate disk name: {}", disk.name));
        }
    }

    // Check for duplicate boot orders
    let mut boot_orders = std::collections::HashSet::new();
    for disk in disks {
        if disk.boot_order > 0 && !boot_orders.insert(disk.boot_order) {
            return Err(anyhow!("Duplicate boot order: {}", disk.boot_order));
        }
    }

    // Validate each disk
    for disk in disks {
        validate_disk(disk)?;
    }

    Ok(())
}

fn validate_disk(disk: &DiskConfig) -> Result<()> {
    if disk.name.is_empty() {
        return Err(anyhow!("Disk name cannot be empty"));
    }

    // Validate size for non-container disks
    match &disk.source {
        DiskSource::Blank | DiskSource::DataVolume { .. } => {
            validate_memory_size(&disk.size)?;
        }
        DiskSource::ContainerDisk { image } => {
            if image.is_empty() {
                return Err(anyhow!("Container disk image cannot be empty"));
            }
        }
        DiskSource::PVC { name } => {
            if name.is_empty() {
                return Err(anyhow!("PVC name cannot be empty"));
            }
        }
    }

    Ok(())
}

fn validate_interfaces(interfaces: &[InterfaceConfig]) -> Result<()> {
    if interfaces.is_empty() {
        return Err(anyhow!("At least one network interface must be configured"));
    }

    // Check for duplicate names
    let mut names = std::collections::HashSet::new();
    for interface in interfaces {
        if !names.insert(&interface.name) {
            return Err(anyhow!("Duplicate interface name: {}", interface.name));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DiskConfig, DiskSource, InterfaceConfig, NetworkType, VMConfigBuilder};

    #[test]
    fn test_validate_name() {
        assert!(validate_name("valid-name").is_ok());
        assert!(validate_name("valid.name").is_ok());
        assert!(validate_name("valid123").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("Invalid-Name").is_err());
        assert!(validate_name("invalid_name").is_err());
    }

    #[test]
    fn test_validate_name_length() {
        // Max length is 253
        let long_name = "a".repeat(253);
        assert!(validate_name(&long_name).is_ok());

        let too_long = "a".repeat(254);
        assert!(validate_name(&too_long).is_err());
    }

    #[test]
    fn test_validate_memory_size() {
        assert!(validate_memory_size("4Gi").is_ok());
        assert!(validate_memory_size("2048Mi").is_ok());
        assert!(validate_memory_size("1Ti").is_ok());
        assert!(validate_memory_size("512M").is_ok());
        assert!(validate_memory_size("8G").is_ok());
        assert!(validate_memory_size("2T").is_ok());
        assert!(validate_memory_size("invalid").is_err());
        assert!(validate_memory_size("4GB").is_err());
        assert!(validate_memory_size("4gi").is_err()); // lowercase not allowed
        assert!(validate_memory_size("").is_err());
    }

    #[test]
    fn test_validate_cpu_cores() {
        let config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(0, 1, 1) // Invalid: 0 cores
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .add_pod_network("default")
            .build();

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_cpu_limit() {
        let config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(128, 2, 2) // 512 total CPUs - exceeds limit
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .add_pod_network("default")
            .build();

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_duplicate_disk_names() {
        let mut config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_blank_disk("disk1", "20Gi", 1)
            .add_blank_disk("disk1", "30Gi", 2) // Duplicate name
            .add_pod_network("default")
            .build();

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_duplicate_boot_order() {
        let mut config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .build();

        config.disks.push(DiskConfig {
            name: "disk1".to_string(),
            size: "20Gi".to_string(),
            storage_class: None,
            boot_order: 1,
            source: DiskSource::Blank,
        });

        config.disks.push(DiskConfig {
            name: "disk2".to_string(),
            size: "30Gi".to_string(),
            storage_class: None,
            boot_order: 1, // Duplicate boot order
            source: DiskSource::Blank,
        });

        config.interfaces.push(InterfaceConfig {
            name: "default".to_string(),
            network: "default".to_string(),
            model: "virtio".to_string(),
            network_type: NetworkType::Pod,
        });

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_no_disks() {
        let mut config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_pod_network("default")
            .build();

        config.disks.clear(); // Remove all disks

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_no_interfaces() {
        let mut config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .build();

        config.interfaces.clear(); // Remove all interfaces

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_duplicate_interface_names() {
        let mut config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .build();

        config.interfaces.push(InterfaceConfig {
            name: "eth0".to_string(),
            network: "default".to_string(),
            model: "virtio".to_string(),
            network_type: NetworkType::Pod,
        });

        config.interfaces.push(InterfaceConfig {
            name: "eth0".to_string(), // Duplicate
            network: "other".to_string(),
            model: "virtio".to_string(),
            network_type: NetworkType::Pod,
        });

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_container_disk_no_image() {
        let mut config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_pod_network("default")
            .build();

        config.disks.push(DiskConfig {
            name: "containerdisk".to_string(),
            size: "0".to_string(),
            storage_class: None,
            boot_order: 1,
            source: DiskSource::ContainerDisk {
                image: "".to_string(), // Empty image
            },
        });

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_pvc_no_name() {
        let mut config = VMConfigBuilder::new("test")
            .namespace("default")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_pod_network("default")
            .build();

        config.disks.push(DiskConfig {
            name: "pvcdisk".to_string(),
            size: "20Gi".to_string(),
            storage_class: None,
            boot_order: 1,
            source: DiskSource::PVC {
                name: "".to_string(), // Empty name
            },
        });

        assert!(validate_vm_config(&config).is_err());
    }

    #[test]
    fn test_validate_valid_complex_config() {
        let config = VMConfigBuilder::new("production-vm")
            .namespace("production")
            .cpu(16, 2, 1)
            .memory("64Gi")
            .add_blank_disk("rootdisk", "100Gi", 1)
            .add_blank_disk("datadisk", "500Gi", 2)
            .add_container_disk("cdrom", "registry.io/iso:latest", 3)
            .add_pod_network("management")
            .label("app", "database")
            .label("tier", "backend")
            .build();

        assert!(validate_vm_config(&config).is_ok());
    }
}
