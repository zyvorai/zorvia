use crate::config::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;

pub static TEMPLATES: Lazy<TemplateManager> = Lazy::new(TemplateManager::new);

/// Manages VM templates
pub struct TemplateManager {
    templates: HashMap<String, VMConfig>,
}

impl TemplateManager {
    pub fn new() -> Self {
        let mut templates = HashMap::new();

        templates.insert("ubuntu".to_string(), ubuntu_template());
        templates.insert("centos".to_string(), centos_template());
        templates.insert("fedora".to_string(), fedora_template());
        templates.insert("rhel".to_string(), rhel_template());
        templates.insert("debian".to_string(), debian_template());
        templates.insert("windows".to_string(), windows_template());

        Self { templates }
    }

    pub fn get(&self, name: &str) -> Option<VMConfig> {
        self.templates.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<_> = self.templates.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn exists(&self, name: &str) -> bool {
        self.templates.contains_key(name)
    }
}

impl Default for TemplateManager {
    fn default() -> Self {
        Self::new()
    }
}

fn ubuntu_template() -> VMConfig {
    VMConfigBuilder::new("ubuntu-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_container_disk(
            "rootdisk",
            "quay.io/containerdisks/ubuntu:22.04",
            1,
        )
        .add_blank_disk("datadisk", "20Gi", 2)
        .add_pod_network("default")
        .label("os", "ubuntu")
        .label("os.version", "22.04")
        .cloud_init(default_ubuntu_cloud_init())
        .build()
}

fn centos_template() -> VMConfig {
    VMConfigBuilder::new("centos-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_container_disk(
            "rootdisk",
            "quay.io/containerdisks/centos-stream:9",
            1,
        )
        .add_blank_disk("datadisk", "20Gi", 2)
        .add_pod_network("default")
        .label("os", "centos")
        .label("os.version", "stream9")
        .cloud_init(default_cloud_init())
        .build()
}

fn fedora_template() -> VMConfig {
    VMConfigBuilder::new("fedora-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_container_disk(
            "rootdisk",
            "quay.io/containerdisks/fedora:39",
            1,
        )
        .add_blank_disk("datadisk", "20Gi", 2)
        .add_pod_network("default")
        .label("os", "fedora")
        .label("os.version", "39")
        .cloud_init(default_cloud_init())
        .build()
}

fn rhel_template() -> VMConfig {
    VMConfigBuilder::new("rhel-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_blank_disk("rootdisk", "30Gi", 1)
        .add_pod_network("default")
        .label("os", "rhel")
        .label("os.version", "9")
        .cloud_init(default_cloud_init())
        .build()
}

fn debian_template() -> VMConfig {
    VMConfigBuilder::new("debian-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_container_disk(
            "rootdisk",
            "quay.io/containerdisks/debian:12",
            1,
        )
        .add_blank_disk("datadisk", "20Gi", 2)
        .add_pod_network("default")
        .label("os", "debian")
        .label("os.version", "12")
        .cloud_init(default_cloud_init())
        .build()
}

fn windows_template() -> VMConfig {
    VMConfigBuilder::new("windows-vm")
        .namespace("default")
        .cpu(4, 1, 1)
        .memory("8Gi")
        .add_blank_disk("rootdisk", "60Gi", 1)
        .add_pod_network("default")
        .label("os", "windows")
        .label("os.version", "2022")
        .build()
}

fn default_cloud_init() -> String {
    r#"#cloud-config
user: zorvia
password: zorvia
chpasswd: { expire: False }
ssh_pwauth: True
package_update: true
packages:
  - qemu-guest-agent
runcmd:
  - [ systemctl, enable, qemu-guest-agent ]
  - [ systemctl, start, qemu-guest-agent ]
"#
    .to_string()
}

fn default_ubuntu_cloud_init() -> String {
    r#"#cloud-config
user: ubuntu
password: ubuntu
chpasswd: { expire: False }
ssh_pwauth: True
package_update: true
packages:
  - qemu-guest-agent
runcmd:
  - [ systemctl, enable, qemu-guest-agent ]
  - [ systemctl, start, qemu-guest-agent ]
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_manager() {
        let manager = TemplateManager::new();

        assert!(manager.exists("ubuntu"));
        assert!(manager.exists("centos"));
        assert!(manager.exists("fedora"));
        assert!(!manager.exists("nonexistent"));

        let ubuntu = manager.get("ubuntu").unwrap();
        assert_eq!(ubuntu.cpu.cores, 2);
        assert_eq!(ubuntu.memory.size, "4Gi");
    }

    #[test]
    fn test_list_templates() {
        let manager = TemplateManager::new();
        let templates = manager.list();

        assert!(templates.contains(&"ubuntu".to_string()));
        assert!(templates.contains(&"fedora".to_string()));
    }
}
