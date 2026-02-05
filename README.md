# 🚀 zorvia

> Craft VMs for KubeVirt with Rust power!

A powerful, ergonomic, and extensible Rust CLI and library to declaratively build, validate, visualize, and apply KubeVirt VMs.

## ✨ Features

- 🎯 **Template-based VM creation** - Pre-configured templates for Ubuntu, CentOS, Fedora, Debian, RHEL, and Windows
- 🛠️ **Flexible configuration** - YAML/JSON configuration files or CLI arguments
- ✅ **Built-in validation** - Validate VM configs before deployment
- 📦 **Cloud-init support** - Easy VM customization with cloud-init
- 🔧 **CLI & Library** - Use as a command-line tool or Rust library
- 🎨 **Multiple output formats** - YAML or JSON output

## 📦 Installation

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/yourusername/zorvia.git
cd zorvia
cargo build --release
```

## 🚀 Quick Start

### Create a VM from a template

```bash
# Create an Ubuntu VM
zorvia create my-ubuntu --template ubuntu --cpus 4 --memory 8Gi

# Create a Fedora VM with custom disk size
zorvia create my-fedora --template fedora --disk-size 40Gi

# Dry run to see the manifest
zorvia create my-vm --template centos --dry-run
```

### List available templates

```bash
zorvia templates
```

### View template details

```bash
zorvia template ubuntu
zorvia template ubuntu --output json
```

### Generate a VM manifest

```bash
# Generate VMConfig format
zorvia generate my-vm --template ubuntu --output vm.yaml

# Generate KubeVirt VirtualMachine CRD (ready to kubectl apply)
zorvia generate web-server \
  --template ubuntu \
  --cpus 8 \
  --memory 16Gi \
  --disk-size 100Gi \
  --kubevirt \
  --output webserver.yaml

# Apply to cluster
kubectl apply -f webserver.yaml
```

### Create from configuration file

```bash
zorvia create my-custom-vm --from-file examples/basic-vm.yaml
```

### Validate a configuration

```bash
zorvia validate examples/ubuntu-cloud-init.yaml
```

### Manage VMs on Kubernetes

```bash
# Create VM on cluster
zorvia create production-db --template ubuntu --cpus 8 --memory 32Gi

# List VMs
zorvia list
zorvia list --all-namespaces

# Get VM details
zorvia get production-db

# Start/Stop/Restart VMs
zorvia start production-db
zorvia stop production-db
zorvia restart production-db

# Delete VM
zorvia delete production-db
zorvia delete production-db --yes  # Skip confirmation
```

## 📋 Available Templates

- **ubuntu** - Ubuntu 22.04 with cloud-init
- **centos** - CentOS Stream 9
- **fedora** - Fedora 39
- **debian** - Debian 12
- **rhel** - Red Hat Enterprise Linux 9
- **windows** - Windows Server 2022

## 🔧 Configuration File Format

Create a YAML or JSON file with your VM configuration:

```yaml
name: my-vm
namespace: default
cpu:
  cores: 4
  sockets: 1
  threads: 1
memory:
  size: 8Gi
disks:
  - name: rootdisk
    size: 40Gi
    boot_order: 1
    storage_class: standard
    source:
      type: Blank
interfaces:
  - name: default
    network: default
    model: virtio
    network_type: Pod
cloud_init:
  user_data: |
    #cloud-config
    user: zorvia
    password: zorvia
    chpasswd: { expire: False }
labels:
  app: my-app
```

## 📚 Library Usage

Use zorvia as a library in your Rust projects:

```rust
use zorvia::config::VMConfigBuilder;
use zorvia::output::{to_yaml, OutputFormat};

fn main() -> anyhow::Result<()> {
    let config = VMConfigBuilder::new("my-vm")
        .namespace("production")
        .cpu(4, 1, 1)
        .memory("8Gi")
        .add_blank_disk("rootdisk", "40Gi", 1)
        .add_pod_network("default")
        .label("app", "webserver")
        .build();

    let yaml = to_yaml(&config)?;
    println!("{}", yaml);

    Ok(())
}
```

## 🎯 Roadmap

### Current Status (v0.1.0)

#### Core Features ✅
- ✅ Core type definitions
- ✅ Template system (6 templates)
- ✅ Configuration validation (29 tests)
- ✅ YAML/JSON output
- ✅ CLI scaffolding (11 commands)

#### Kubernetes Integration ✅
- ✅ Full Kubernetes CRUD operations
- ✅ VM lifecycle management (create, start, stop, restart, delete)
- ✅ KubeVirt CRD types and conversion
- ✅ VM status monitoring
- ✅ Multi-namespace support
- ✅ PVC creation support

#### Manifest Generation ✅
- ✅ KubeVirt VirtualMachine CRD output
- ✅ Cloud-init volume generation
- ✅ All disk types (Blank, PVC, ContainerDisk, DataVolume)
- ✅ All network types (Pod, Bridge, Multus)

### Future Enhancements

- 🔌 Pluggable template registry (local/remote)
- 📸 VM snapshots and backups
- 🔄 Live migration support
- 🎨 Interactive TUI for monitoring
- 🔧 Terraform provider
- 🌐 Advanced networking (SR-IOV, OVN)
- 💾 DataVolume CRD management (CDI)
- 🚀 VM presets and profiles
- 📊 Resource quota management
- 🔐 RBAC and security policies

## 🧪 Development

### Run tests

```bash
cargo test
```

### Run with debug logging

```bash
zorvia --verbose create my-vm --template ubuntu
```

### Build documentation

```bash
cargo doc --open
```

## 📖 Examples

See the `examples/` directory for more configuration examples:

- `basic-vm.yaml` - Simple VM configuration
- `ubuntu-cloud-init.yaml` - Ubuntu VM with cloud-init

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under Apache-2.0.

## 🙏 Acknowledgments

- [KubeVirt](https://kubevirt.io/) - Kubernetes Virtualization API
- [kube-rs](https://github.com/kube-rs/kube) - Kubernetes client for Rust
