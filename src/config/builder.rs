use super::types::*;
use std::collections::HashMap;

/// Builder for VMConfig with fluent API
#[derive(Debug, Default)]
pub struct VMConfigBuilder {
    config: VMConfig,
}

impl VMConfigBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: VMConfig {
                name: name.into(),
                namespace: "default".to_string(),
                ..Default::default()
            },
        }
    }

    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.config.namespace = namespace.into();
        self
    }

    pub fn cpu(mut self, cores: u32, sockets: u32, threads: u32) -> Self {
        self.config.cpu = CPUConfig {
            cores,
            sockets,
            threads,
            model: None,
        };
        self
    }

    pub fn cpu_model(mut self, model: impl Into<String>) -> Self {
        self.config.cpu.model = Some(model.into());
        self
    }

    pub fn memory(mut self, size: impl Into<String>) -> Self {
        self.config.memory = MemoryConfig { size: size.into() };
        self
    }

    pub fn add_disk(mut self, disk: DiskConfig) -> Self {
        self.config.disks.push(disk);
        self
    }

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

    pub fn add_interface(mut self, interface: InterfaceConfig) -> Self {
        self.config.interfaces.push(interface);
        self
    }

    pub fn add_pod_network(mut self, name: impl Into<String>) -> Self {
        self.config.interfaces.push(InterfaceConfig {
            name: name.into(),
            network: "default".to_string(),
            model: "virtio".to_string(),
            network_type: NetworkType::Pod,
        });
        self
    }

    pub fn cloud_init(mut self, user_data: impl Into<String>) -> Self {
        self.config.cloud_init = Some(CloudInitConfig {
            user_data: user_data.into(),
            network_data: None,
        });
        self
    }

    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.labels.insert(key.into(), value.into());
        self
    }

    pub fn labels(mut self, labels: HashMap<String, String>) -> Self {
        self.config.labels = labels;
        self
    }

    pub fn annotation(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.annotations.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> VMConfig {
        self.config
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
}
