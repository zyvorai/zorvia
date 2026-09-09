# Zorvia

[![CI](https://github.com/zyvorai/zorvia/workflows/CI/badge.svg)](https://github.com/zyvorai/zorvia/actions)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![KubeVirt](https://img.shields.io/badge/KubeVirt-native-6d28d9.svg)](https://kubevirt.io/)

**Craft VMs. Day-2 them. One control plane.**

Zorvia is the KubeVirt operator’s toolkit — a fast Rust CLI, an interactive TUI, and a signed-in web console — for creating, running, and day-2 managing virtual machines on Kubernetes.

```text
  template + profile  →  VirtualMachine  →  Running
       CLI / TUI / Web console                 │
                                               ├─ serial · VNC · in-browser SSH
                                               ├─ expose SSH · VNC · RDP
                                               ├─ snapshot · clone · pause
                                               └─ drift · plan · guest-insight
```

Apache License 2.0 only · [zyvorai/zorvia](https://github.com/zyvorai/zorvia) · [zyvor.dev](https://zyvor.dev)

---

## Why Zorvia

| You need | Zorvia gives you |
|----------|------------------|
| Ship a VM without writing a CRD by hand | **43 OS templates** + **8 resource profiles** |
| Stand up a whole stack | **Blueprints** (LAMP, 3-tier, k8s-cluster, CI/CD, …) |
| Operate from a browser | **Web console** — create, power, console, expose, snapshots |
| Reach the guest | Serial, VNC, and in-browser SSH over authenticated WebSockets |
| Catch config drift | **Drift Guard** + **Change Planner** (`zorvia drift` / `zorvia plan`) |
| Seed a golden library | **CDI image bundles** + quay.io containerdisks |
| Drive it from Terraform | Scaffold + module over the same Fabric HTTP API |
| Windows control plane | **Kryton** proxy when `KRYTON_URL` is set (`/app/windows`) |
| Stay close to the metal | **CLI + TUI** on the same cluster APIs |

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
(`web` is the default feature — Axum SPA/API. PAM auth is stubbed and not production-ready.)

### Create a VM (CLI)

```bash
zorvia recommend database
zorvia create prod-db --template ubuntu-22.04 --profile database
zorvia start prod-db
zorvia wait-ready prod-db --timeout 120
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
| `/app/vms/:name/console` | Serial + VNC + SSH tabs |
| `/app/snapshots` | Snapshot browser |
| `/app/windows` | Kryton Windows plane (when enabled) |

```text
wss://<HOST>:30152/ws/console/<vm>?token=<jwt>
wss://<HOST>:30152/ws/vnc/<vm>?token=<jwt>
wss://<HOST>:30152/ws/ssh/<vm>?token=<jwt>&user=ubuntu
```

Expose guest **SSH / VNC / RDP** as NodePort Services (`ZORVIA_EXPOSE_HOST` sets the host shown in the UI). Full map: [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md).

---

## Ports

| Surface | Port | Notes |
|---------|------|--------|
| Lab / cluster HTTPS | **30152** | Service NodePort → UI + API |
| In-pod / `api-serve` | **5151** | Container listen port in `deploy/k8s.yaml` |
| Config default | **8080** | `~/.config/zorvia/config.toml` listen default if you do not pass `--port` |

---

## Day-2 surface

**Lifecycle** — create, start, stop, restart, pause, resume, clone, export, delete  
**Console** — serial, VNC, in-browser SSH (needs `ssh` or `virtctl` on the API pod)  
**Guests** — NodePort expose, cloud-init, wait-ready, wait-image, guest-insight, health  
**Scale-out** — profiles, blueprints, workload recommendations  
**Surfaces** — CLI · TUI · web SPA · Fabric-compatible HTTP API · Rust library

```bash
zorvia list
zorvia get prod-db
zorvia pause prod-db
zorvia resume prod-db
zorvia clone prod-db staging-db --start
zorvia snapshot-create prod-db --name before-upgrade
zorvia guest-insight prod-db
zorvia health prod-db --detailed
zorvia tui --interactive
zorvia api-serve --tls --port 5151
```

---

## Ops that differentiate

### Drift Guard & Change Planner

```bash
zorvia drift desired.yaml                         # vs live cluster
zorvia drift desired.yaml --actual live.yaml -o json
zorvia plan desired.yaml --vm payments-01         # conservative execution plan
zorvia plan desired.yaml --actual live.yaml --fail-on-downtime
```

Details: [docs/DRIFT_GUARD.md](docs/DRIFT_GUARD.md) · [docs/CHANGE_PLANNER.md](docs/CHANGE_PLANNER.md)

### Golden images & CDI

```bash
zorvia image-bundle --distro ubuntu --version 22.04 --storage-class fast
# or: STORAGE_CLASS=fast ./fixtures/golden-images/generate-bundles.sh
```

Catalog: [docs/GOLDEN_IMAGES.md](docs/GOLDEN_IMAGES.md)

### Terraform (Fabric API)

```bash
zorvia terraform-scaffold --output ./terraform/zorvia-vm --url https://HOST:30152
# module: terraform/modules/zorvia_vm
```

No HashiCorp registry provider binary yet — scaffold + `schema.json` ship today. [docs/TERRAFORM.md](docs/TERRAFORM.md)

### Kryton (Windows)

Set `KRYTON_URL` (and usually `KRYTON_TOKEN` + `KRYTON_PROJECT`) on the API pod. The SPA gains `/app/windows`; credentials never leave the server. [docs/KRYTON_INTEGRATION.md](docs/KRYTON_INTEGRATION.md)

### vCenter-style inventory

```bash
zorvia inventory
zorvia activity
zorvia maintenance-plan NODE
zorvia placement-advisor --cpu 2 --memory-gib 4
```

Matrix: [docs/VCENTER_FEATURE_MATRIX.md](docs/VCENTER_FEATURE_MATRIX.md)

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

**43 named templates** (including aliases) across Ubuntu, Fedora, CentOS Stream, Debian, RHEL, Alma, Rocky, OpenSUSE, Alpine, Arch, Oracle, FreeBSD, Flatcar, Talos, and Windows.

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

Architecture notes and deeper CLI surfaces (FinOps, mesh-style libraries, etc.): [DEVELOPMENT.md](DEVELOPMENT.md).

---

## Docs

| Doc | Topic |
|-----|--------|
| [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md) | SPA, HTTP API, WebSockets, expose |
| [docs/KRYTON_INTEGRATION.md](docs/KRYTON_INTEGRATION.md) | Windows plane via Kryton |
| [docs/TERRAFORM.md](docs/TERRAFORM.md) | Scaffold + Fabric API module |
| [docs/DRIFT_GUARD.md](docs/DRIFT_GUARD.md) | Desired-vs-live drift |
| [docs/CHANGE_PLANNER.md](docs/CHANGE_PLANNER.md) | Plans from drift |
| [docs/GUEST_INSIGHT.md](docs/GUEST_INSIGHT.md) | QEMU Guest Agent readiness |
| [docs/GOLDEN_IMAGES.md](docs/GOLDEN_IMAGES.md) | quay + CDI golden bundles |
| [docs/OS_TEMPLATES.md](docs/OS_TEMPLATES.md) | Template catalog |
| [docs/SNAPSHOTS.md](docs/SNAPSHOTS.md) | Snapshots |
| [docs/INTERACTIVE_TUI.md](docs/INTERACTIVE_TUI.md) | Interactive TUI |
| [QUICK_REFERENCE.md](QUICK_REFERENCE.md) | Command card |
| [docs/README.md](docs/README.md) | Full index |
| [CHANGELOG.md](CHANGELOG.md) | What’s new |

---

## Security

Secure-by-default defaults: no `unsafe`, CORS off unless configured, TLS verification enforced, path traversal guards on profile/blueprint storage, sanitized HTTP errors, bounded auth inputs. Report issues privately via [GitHub Security Advisories](https://github.com/zyvorai/zorvia/security/advisories) or `info@zyvor.dev` — see [SECURITY.md](SECURITY.md).

---

## Roadmap (near-term)

- Published HashiCorp registry provider binary (module + `schema.json` ship today)

---

## Contributing

PRs welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Copyright 2026 ZyvorAI Labs Private Limited.

Licensed under the [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0) — see [LICENSE](LICENSE) and [NOTICE](NOTICE). **Apache-2.0 only** (no MIT dual-license).

## Acknowledgments

[KubeVirt](https://kubevirt.io/) · [kube-rs](https://github.com/kube-rs/kube)
