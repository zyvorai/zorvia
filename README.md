# Zorvia

[![CI](https://github.com/zyvorai/zorvia/workflows/CI/badge.svg)](https://github.com/zyvorai/zorvia/actions)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![KubeVirt](https://img.shields.io/badge/KubeVirt-native-6d28d9.svg)](https://kubevirt.io/)

**Craft VMs. One control plane.**

Zorvia is the KubeVirt operator’s toolkit — a fast Rust CLI, an interactive TUI, and a signed-in web console — for creating, running, and day-2 managing virtual machines on Kubernetes.

```text
  template + profile  →  VirtualMachine  →  Running
       CLI / TUI / Web console                 │
                                               ├─ serial console + VNC
                                               ├─ expose SSH · VNC · RDP
                                               └─ snapshot · clone · power
```

Apache License 2.0 only · [zyvorai/zorvia](https://github.com/zyvorai/zorvia) · [zyvor.dev](https://zyvor.dev)

---

## Why Zorvia

| You need | Zorvia gives you |
|----------|------------------|
| Ship a VM without writing a CRD by hand | **44 OS templates** + **8 resource profiles** |
| Stand up a whole stack | **Blueprints** (LAMP, 3-tier, k8s-cluster, CI/CD, …) |
| Operate from a browser | **Web console** — create, power, console, expose, snapshots |
| Stay close to the metal | **CLI + TUI** on the same cluster APIs |
| Trust the supply chain | **Safe Rust**, validated configs, sanitized API errors |

No OpenShift tax. No raw YAML required. Same VMs whether you use the terminal or `https://host:30152/app`.

---

## Quick start

### Install

```bash
git clone https://github.com/zyvorai/zorvia.git
cd zorvia
cargo build --release --features web
# binary: target/release/zorvia
```

Or from a checkout: `cargo install --path . --features web`

### Create a VM (CLI)

```bash
zorvia recommend database
zorvia create prod-db --template ubuntu-22.04 --profile database
zorvia start prod-db
zorvia status prod-db --watch
```

### Deploy a stack

```bash
zorvia blueprints
zorvia deploy lamp --prefix myapp --start
zorvia list
```

### Open the web console

In-cluster HTTPS on NodePort **30152** (self-signed in lab — use `curl -sk`):

```bash
kubectl apply -f deploy/k8s.yaml
# or ship a lab: ./deploy/remote-deploy.sh <host> sus --quick

open https://<HOST>:30152/app
```

| Path | What |
|------|------|
| `/app` | Dashboard |
| `/app/create` | Create VM (Linux cloud-init / Windows) |
| `/app/vms/:name` | Details, power, port-forwards, snapshots |
| `/app/vms/:name/console` | Serial terminal + VNC |
| `/app/snapshots` | Snapshot browser |

Serial and VNC proxy through authenticated WebSockets to KubeVirt:

```text
wss://<HOST>:30152/ws/console/<vm>?token=<jwt>
wss://<HOST>:30152/ws/vnc/<vm>?token=<jwt>
```

Expose guest **SSH / VNC / RDP** as NodePort Services (`ZORVIA_EXPOSE_HOST` sets the host shown in the UI). Full map: [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md).

---

## What you can do

**Lifecycle** — create, start, stop, restart, clone, export, delete  
**Day-2** — serial console, VNC, NodePort expose, snapshots, cloud-init  
**Scale-out** — profiles, blueprints, health scores, workload recommendations  
**Surfaces** — CLI · TUI · web SPA · Fabric-compatible HTTP API · Rust library

```bash
zorvia list
zorvia get prod-db
zorvia clone prod-db staging-db --start
zorvia snapshot-create prod-db --name before-upgrade
zorvia health prod-db --detailed
zorvia tui --interactive
zorvia api-serve --tls --port 5151
```

---

## Profiles

Eight built-in shapes — pick a workload, not a spreadsheet:

| Profile | CPU | Memory | Disk | Best for |
|---------|-----|--------|------|----------|
| minimal | 1 | 512Mi | 5Gi | Agents, jump hosts |
| dev | 1 | 2Gi | 10Gi | Local / learning |
| test | 2 | 4Gi | 20Gi | CI pipelines |
| web | 4 | 8Gi | 40Gi | Nginx, static sites |
| prod | 4 | 8Gi | 40Gi | Production apps |
| database | 6 | 16Gi | 200Gi | Postgres, MySQL, Mongo |
| microservice | 2 | 4Gi | 20Gi | Container / node roles |
| high-perf | 8 | 16Gi | 100Gi | ML, heavy I/O |

```bash
zorvia profiles
zorvia profile database
zorvia create api --template fedora-40 --profile web
```

---

## Blueprints

| Blueprint | VMs | Stack |
|-----------|-----|--------|
| lamp | 2 | MySQL + Apache |
| k8s-cluster | 3 | Control plane + workers |
| 3tier | 3 | DB + app + Nginx |
| cicd | 3 | GitLab + Jenkins + registry |
| dev-stack | 3 | DB + Redis + workspace |

```bash
zorvia blueprint lamp
zorvia deploy lamp --prefix demo --dry-run
zorvia deploy lamp --prefix demo --start
```

---

## Templates

**44 OS templates** across Ubuntu, Fedora, CentOS Stream, Debian, RHEL, Alma, Rocky, OpenSUSE, Alpine, Arch, Oracle, FreeBSD, Flatcar, Talos, and Windows.

```bash
zorvia templates
zorvia template ubuntu-22.04
zorvia create web1 --template almalinux-9 --cpus 4 --memory 8Gi
```

Catalog: [docs/OS_TEMPLATES.md](docs/OS_TEMPLATES.md)

---

## Config & library

**Declarative VM** (`examples/ubuntu-cloud-init.yaml` style):

```yaml
name: my-vm
namespace: default
cpu: { cores: 4, sockets: 1, threads: 1 }
memory: { size: 8Gi }
disks:
  - name: rootdisk
    size: 40Gi
    boot_order: 1
    source: { type: Blank }
interfaces:
  - name: default
    network: default
    model: virtio
    network_type: Pod
cloud_init:
  user_data: |
    #cloud-config
    users:
      - name: zorvia
        sudo: ALL=(ALL) NOPASSWD:ALL
        ssh_authorized_keys:
          - ssh-ed25519 AAAA…
```

```bash
zorvia validate examples/ubuntu-cloud-init.yaml
zorvia create my-vm --from-file examples/basic-vm.yaml
zorvia generate web --template ubuntu --kubevirt -o web.yaml
```

**App defaults** (CLI wins over files):

```bash
zorvia config-init
zorvia config-show
# ~/.config/zorvia/config.toml  ·  /etc/zorvia/config.toml
```

**As a crate:**

```rust
use zorvia::config::VMConfigBuilder;
use zorvia::output::to_yaml;

fn main() -> anyhow::Result<()> {
    let cfg = VMConfigBuilder::new("my-vm")
        .namespace("production")
        .cpu(4, 1, 1)
        .memory("8Gi")
        .add_blank_disk("rootdisk", "40Gi", 1)
        .add_pod_network("default")
        .label("app", "webserver")
        .build();
    println!("{}", to_yaml(&cfg)?);
    Ok(())
}
```

---

## Develop

```bash
make help      # tasks
make test      # suite
make ci        # format + clippy + test
make release   # optimized binary
make tui       # interactive TUI
zorvia commands
```

Lab ship (build image, apply `deploy/k8s.yaml`, NodePort 30152):

```bash
./deploy/remote-deploy.sh <host> sus --quick
```

---

## Docs

| Doc | Topic |
|-----|--------|
| [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md) | SPA, HTTP API, WebSockets, expose |
| [docs/INNOVATIVE_FEATURES.md](docs/INNOVATIVE_FEATURES.md) | Profiles, blueprints, health |
| [docs/OS_TEMPLATES.md](docs/OS_TEMPLATES.md) | Template catalog |
| [docs/SNAPSHOTS.md](docs/SNAPSHOTS.md) | Snapshots |
| [docs/INTERACTIVE_TUI.md](docs/INTERACTIVE_TUI.md) | TUI |
| [QUICK_REFERENCE.md](QUICK_REFERENCE.md) | Command card |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Architecture |
| [CHANGELOG.md](CHANGELOG.md) | What’s new |
| [docs/README.md](docs/README.md) | Full index |

---

## Security

Secure-by-default defaults: no `unsafe`, CORS off unless configured, TLS verification enforced, path traversal guards on profile/blueprint storage, sanitized HTTP errors, bounded auth inputs. Report issues privately via [GitHub Security Advisories](https://github.com/zyvorai/zorvia/security/advisories) or `info@zyvor.dev` — see [SECURITY.md](SECURITY.md).

---

## Roadmap (near-term)

- Published HashiCorp registry provider binary (module + schema.json ship today)  

---

## Contributing

PRs welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Copyright 2026 ZyvorAI Labs Private Limited.

Licensed under the [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0) — see [LICENSE](LICENSE) and [NOTICE](NOTICE). **Apache-2.0 only** (no MIT dual-license).

## Acknowledgments

[KubeVirt](https://kubevirt.io/) · [kube-rs](https://github.com/kube-rs/kube)
