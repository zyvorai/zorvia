//! Example of using zorvia as a library

use zorvia::config::{InterfaceConfig, NetworkType, VMConfigBuilder};
use zorvia::output::to_yaml;
use zorvia::templates::TEMPLATES;

fn main() -> anyhow::Result<()> {
    println!("=== Example 1: Building a VM from scratch ===\n");

    let config = VMConfigBuilder::new("web-server")
        .namespace("production")
        .cpu(4, 1, 1)
        .memory("8Gi")
        .add_blank_disk("rootdisk", "40Gi", 1)
        .add_pod_network("default")
        .label("app", "nginx")
        .label("tier", "frontend")
        .cloud_init(
            r#"#cloud-config
user: webadmin
password: securepass
chpasswd: { expire: False }
packages:
  - nginx
  - certbot
runcmd:
  - systemctl enable nginx
  - systemctl start nginx
"#,
        )
        .build();

    let yaml = to_yaml(&config)?;
    println!("{}\n", yaml);

    println!("=== Example 2: Using a template as a base ===\n");

    let mut ubuntu_vm = TEMPLATES.get("ubuntu").expect("ubuntu template exists");
    ubuntu_vm.name = "database-server".to_string();
    ubuntu_vm.namespace = "production".to_string();
    ubuntu_vm.cpu.cores = 8;
    ubuntu_vm.memory.size = "16Gi".to_string();
    ubuntu_vm
        .labels
        .insert("app".to_string(), "postgresql".to_string());

    let yaml = to_yaml(&ubuntu_vm)?;
    println!("{}\n", yaml);

    println!("=== Example 3: Creating a VM with container disk ===\n");

    let config = VMConfigBuilder::new("dev-vm")
        .namespace("development")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_container_disk("containerdisk", "quay.io/containerdisks/fedora:39", 1)
        .add_blank_disk("workspace", "50Gi", 2)
        .add_pod_network("default")
        .label("environment", "dev")
        .build();

    let yaml = to_yaml(&config)?;
    println!("{}\n", yaml);

    println!("=== Example 4: Advanced networking ===\n");

    let mut config = VMConfigBuilder::new("network-vm")
        .namespace("default")
        .cpu(2, 1, 1)
        .memory("4Gi")
        .add_blank_disk("rootdisk", "20Gi", 1)
        .build();

    // Add multiple network interfaces
    config.interfaces.push(InterfaceConfig {
        name: "management".to_string(),
        network: "management-network".to_string(),
        model: "virtio".to_string(),
        network_type: NetworkType::Pod,
    });

    config.interfaces.push(InterfaceConfig {
        name: "data".to_string(),
        network: "nad-data-network".to_string(),
        model: "virtio".to_string(),
        network_type: NetworkType::Multus {
            name: "data-network".to_string(),
        },
    });

    let yaml = to_yaml(&config)?;
    println!("{}\n", yaml);

    println!("=== Example 5: Listing available templates ===\n");

    for template_name in TEMPLATES.list() {
        println!("- {}", template_name);
        if let Some(template) = TEMPLATES.get(&template_name) {
            println!("  CPU: {} cores", template.cpu.cores);
            println!("  Memory: {}", template.memory.size);
            println!("  Disks: {}", template.disks.len());
            println!();
        }
    }

    Ok(())
}
