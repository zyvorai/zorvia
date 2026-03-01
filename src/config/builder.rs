use super::types::*;
use std::collections::HashMap;

/// Fluent builder for constructing [`VMConfig`] instances.
///
/// # Example
/// ```
/// use zorvia::VMConfigBuilder;
///
/// let config = VMConfigBuilder::new("my-vm")
///     .namespace("production")
///     .cpu(4, 1, 1)
///     .memory("8Gi")
///     .add_blank_disk("rootdisk", "40Gi", 1)
///     .add_pod_network("eth0")
///     .label("app", "web")
///     .build();
///
/// assert_eq!(config.name, "my-vm");
/// assert_eq!(config.cpu.cores, 4);
/// ```
#[derive(Debug, Default)]
pub struct VMConfigBuilder {
    config: VMConfig,
}

impl VMConfigBuilder {
    /// Create a new builder with the given VM name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: VMConfig {
                name: name.into(),
                namespace: "default".to_string(),
                ..Default::default()
            },
        }
    }

    /// Set the Kubernetes namespace for the VM.
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.config.namespace = namespace.into();
        self
    }

    /// Set the CPU topology (cores, sockets, threads).
    pub fn cpu(mut self, cores: u32, sockets: u32, threads: u32) -> Self {
        self.config.cpu = CPUConfig {
            cores,
            sockets,
            threads,
            model: None,
        };
        self
    }

    /// Set the CPU model (e.g., "host-passthrough", "Haswell").
    pub fn cpu_model(mut self, model: impl Into<String>) -> Self {
        self.config.cpu.model = Some(model.into());
        self
    }

    /// Set the memory size (e.g., "4Gi", "512Mi").
    pub fn memory(mut self, size: impl Into<String>) -> Self {
        self.config.memory = MemoryConfig { size: size.into() };
        self
    }

    /// Add a pre-configured disk to the VM.
    pub fn add_disk(mut self, disk: DiskConfig) -> Self {
        self.config.disks.push(disk);
        self
    }

    /// Add an empty (blank) disk with the given size and boot order.
    pub fn add_blank_disk(
        mut self,
        name: impl Into<String>,
        size: impl Into<String>,
        boot_order: u32,
    ) -> Self {
        self.config.disks.push(DiskConfig {
            name: name.into(),
            size: size.into(),
            storage_class: None,
            boot_order,
            source: DiskSource::Blank,
        });
        self
    }

    /// Add a container disk from an OCI image (e.g., `quay.io/containerdisks/fedora:39`).
    pub fn add_container_disk(
        mut self,
        name: impl Into<String>,
        image: impl Into<String>,
        boot_order: u32,
    ) -> Self {
        self.config.disks.push(DiskConfig {
            name: name.into(),
            size: "0".to_string(), // Container disks don't need size
            storage_class: None,
            boot_order,
            source: DiskSource::ContainerDisk {
                image: image.into(),
            },
        });
        self
    }

    /// Add a pre-configured network interface.
    pub fn add_interface(mut self, interface: InterfaceConfig) -> Self {
        self.config.interfaces.push(interface);
        self
    }

    /// Add a pod network interface with masquerade binding.
    pub fn add_pod_network(mut self, name: impl Into<String>) -> Self {
        self.config.interfaces.push(InterfaceConfig {
            name: name.into(),
            network: "default".to_string(),
            model: "virtio".to_string(),
            network_type: NetworkType::Pod,
        });
        self
    }

    /// Add a bridge network interface.
    pub fn add_bridge_network(mut self, name: impl Into<String>) -> Self {
        self.config.interfaces.push(InterfaceConfig {
            name: name.into(),
            network: "default".to_string(),
            model: "virtio".to_string(),
            network_type: NetworkType::Bridge,
        });
        self
    }

    /// Add a Multus CNI network interface with the given network attachment definition.
    pub fn add_multus_network(
        mut self,
        name: impl Into<String>,
        network_name: impl Into<String>,
    ) -> Self {
        let net_name = network_name.into();
        self.config.interfaces.push(InterfaceConfig {
            name: name.into(),
            network: net_name.clone(),
            model: "virtio".to_string(),
            network_type: NetworkType::Multus { name: net_name },
        });
        self
    }

    /// Set cloud-init user data for VM initialization.
    pub fn cloud_init(mut self, user_data: impl Into<String>) -> Self {
        self.config.cloud_init = Some(CloudInitConfig {
            user_data: user_data.into(),
            network_data: None,
        });
        self
    }

    /// Add a Kubernetes label to the VM.
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.labels.insert(key.into(), value.into());
        self
    }

    /// Replace all labels with the given map.
    pub fn labels(mut self, labels: HashMap<String, String>) -> Self {
        self.config.labels = labels;
        self
    }

    /// Add a Kubernetes annotation to the VM.
    pub fn annotation(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.annotations.insert(key.into(), value.into());
        self
    }

    /// Build the VMConfig without validation.
    pub fn build(self) -> VMConfig {
        self.config
    }

    /// Build and validate the VMConfig, returning an error if validation fails.
    ///
    /// Validates VM name, namespace, CPU topology, memory size, disk sizes,
    /// and network interface requirements.
    pub fn build_validated(self) -> anyhow::Result<VMConfig> {
        let config = self.config;
        super::validator::validate_vm_config(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let config = VMConfigBuilder::new("test-vm")
            .namespace("test")
            .cpu(2, 1, 1)
            .memory("4Gi")
            .add_blank_disk("rootdisk", "20Gi", 1)
            .add_pod_network("eth0")
            .label("app", "test")
            .build();

        assert_eq!(config.name, "test-vm");
        assert_eq!(config.namespace, "test");
        assert_eq!(config.cpu.cores, 2);
        assert_eq!(config.memory.size, "4Gi");
        assert_eq!(config.disks.len(), 1);
        assert_eq!(config.interfaces.len(), 1);
        assert_eq!(config.labels.get("app"), Some(&"test".to_string()));
    }

    #[test]
    fn test_builder_multus_network() {
        let config = VMConfigBuilder::new("multus-vm")
            .namespace("default")
            .cpu(1, 1, 1)
            .memory("2Gi")
            .add_multus_network("net1", "my-bridge-net")
            .build();

        assert_eq!(config.interfaces.len(), 1);
        assert_eq!(config.interfaces[0].name, "net1");
        assert_eq!(
            config.interfaces[0].network_type,
            NetworkType::Multus {
                name: "my-bridge-net".to_string()
            }
        );
    }

    #[test]
    fn test_builder_bridge_network() {
        let config = VMConfigBuilder::new("bridge-vm")
            .namespace("default")
            .cpu(1, 1, 1)
            .memory("2Gi")
            .add_bridge_network("br0")
            .build();

        assert_eq!(config.interfaces.len(), 1);
        assert_eq!(config.interfaces[0].name, "br0");
        assert_eq!(config.interfaces[0].network_type, NetworkType::Bridge);
    }

    #[test]
    fn test_builder_multiple_networks() {
        let config = VMConfigBuilder::new("multi-net-vm")
            .namespace("default")
            .cpu(1, 1, 1)
            .memory("2Gi")
            .add_pod_network("eth0")
            .add_multus_network("net1", "storage-net")
            .add_bridge_network("br0")
            .build();

        assert_eq!(config.interfaces.len(), 3);
        assert_eq!(config.interfaces[0].network_type, NetworkType::Pod);
        assert_eq!(
            config.interfaces[1].network_type,
            NetworkType::Multus {
                name: "storage-net".to_string()
            }
        );
        assert_eq!(config.interfaces[2].network_type, NetworkType::Bridge);
    }

    #[test]
    fn test_builder_validated_success() {
        let result = VMConfigBuilder::new("valid-vm")
            .namespace("default")
            .cpu(1, 1, 1)
            .memory("1Gi")
            .add_blank_disk("root", "20Gi", 1)
            .add_pod_network("eth0")
            .build_validated();
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_validated_failure() {
        let result = VMConfigBuilder::new("")
            .namespace("default")
            .cpu(1, 1, 1)
            .memory("1Gi")
            .build_validated();
        assert!(result.is_err());
    }
}
