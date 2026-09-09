# Zorvia

[![CI](https://github.com/zyvorai/zorvia/workflows/CI/badge.svg)](https://github.com/zyvorai/zorvia/actions)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![KubeVirt](https://img.shields.io/badge/KubeVirt-native-6d28d9.svg)](https://kubevirt.io/)

**KubeVirt VMs without the YAML tax.**

Zorvia is a Rust toolkit for crafting and day-2 managing virtual machines on Kubernetes:
CLI, interactive TUI, and a signed-in web console on the same Fabric-compatible API.

[Repository](https://github.com/zyvorai/zorvia) · [zyvor.dev](https://zyvor.dev) · Apache-2.0 only · [Changelog](CHANGELOG.md)

---

## Install

```bash
git clone https://github.com/zyvorai/zorvia.git
cd zorvia
cargo install --path . --features web   # `web` is the default feature
# binary also at: cargo build --release --features web → target/release/zorvia
```

Requires Rust **1.76+**, a kubeconfig, and a cluster with **KubeVirt** (CDI optional for golden images / CDI clone).

---

## Quick start

```bash
# Size from a profile, then create (create takes explicit resources)
zorvia profile database
zorvia create prod-db \
  --template ubuntu-22.04 \
  --cpus 6 --memory 16Gi --disk-size 200Gi
zorvia start prod-db
zorvia wait-ready prod-db --timeout 120
zorvia guest-insight prod-db
zorvia status prod-db --watch
```

Deploy a multi-VM stack:

```bash
zorvia blueprints
zorvia deploy lamp --prefix myapp --start
zorvia list
```

Open the lab web console (HTTPS NodePort **30152**, self-signed — use `curl -sk`):

```bash
kubectl apply -f deploy/k8s.yaml
# or: ./deploy/remote-deploy.sh <host> sus --quick
open https://<HOST>:30152/app
```

| Port | Role |
|------|------|
| **30152** | Cluster front door (UI + API) |
| **5151** | In-pod listen / `zorvia api-serve --tls --port 5151` |
| **8080** | Config-file default when `--port` is omitted |

---

## Why Zorvia

| Need | Zorvia |
|------|--------|
| Skip hand-written VM CRDs | **43** named OS templates |
| Size by workload | **8** resource profiles (apply via `--cpus` / `--memory` / `--disk-size`, or blueprints) |
| Ship a stack | **5** blueprints (LAMP, 3-tier, k8s-cluster, CI/CD, dev-stack) |
| Browser ops | Web console — create, power, console, expose, snapshots |
| Reach the guest | Serial, VNC, in-browser SSH (authenticated WebSockets) |
| Catch drift | `zorvia drift` + `zorvia plan` |
| Golden library | quay.io containerdisks + CDI `image-bundle` |
| GitOps | Terraform scaffold + module over the Fabric API |
| Windows plane | Kryton proxy when `KRYTON_URL` is set (`/app/windows`) |

No OpenShift tax. Same VirtualMachines from terminal or browser.

```text
  template  →  VirtualMachine  →  Running
  CLI / TUI / https://host:30152/app
       │
       ├─ serial · VNC · SSH
       ├─ NodePort expose (SSH / VNC / RDP)
       ├─ snapshot · clone · pause · resume
       └─ drift · plan · guest-insight · health
```

---

## Day-2 commands

```bash
zorvia list
zorvia get prod-db
zorvia pause prod-db && zorvia resume prod-db
zorvia clone prod-db staging-db --start
zorvia snapshot-create prod-db --name before-upgrade
zorvia health prod-db --detailed
zorvia drift desired.yaml
zorvia plan desired.yaml --vm prod-db
zorvia tui --interactive
```

---

## Web console & API

| Route | Purpose |
|-------|---------|
| `/app` | Dashboard |
| `/app/create` | Linux (cloud-init) or Windows create |
| `/app/vms/:name` | Power, port-forwards, cloud-init, snapshots |
| `/app/vms/:name/console` | Serial · VNC · SSH |
| `/app/snapshots` | Snapshot browser |
| `/app/windows` | Kryton Windows inventory (optional) |

```text
wss://<HOST>:30152/ws/console/<vm>?token=<jwt>
wss://<HOST>:30152/ws/vnc/<vm>?token=<jwt>
wss://<HOST>:30152/ws/ssh/<vm>?token=<jwt>&user=ubuntu
```

Set `ZORVIA_EXPOSE_HOST` so the UI shows the right host for NodePort SSH/VNC/RDP.
Details: [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md).

```bash
TOKEN=$(curl -sk -X POST https://HOST:30152/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"Admin@321"}' | jq -r .token)
curl -sk -H "Authorization: Bearer $TOKEN" https://HOST:30152/api/vms
```

Lab bootstrap credentials are for labs only — change them before anything shared.

---

## Profiles · blueprints · templates

**Profiles** — look up a shape, then pass resources on `create`:

| Profile | CPU | Memory | Disk | Best for |
|---------|-----|--------|------|----------|
| minimal | 1 | 512Mi | 5Gi | Agents, jump hosts |
| dev | 1 | 2Gi | 10Gi | Learning |
| test | 2 | 4Gi | 20Gi | CI |
| web | 4 | 8Gi | 40Gi | Nginx, static |
| prod | 4 | 8Gi | 40Gi | Production apps |
| database | 6 | 16Gi | 200Gi | Databases |
| microservice | 2 | 4Gi | 20Gi | Node roles |
| high-perf | 8 | 16Gi | 100Gi | ML / heavy I/O |

```bash
zorvia profiles
zorvia profile web
zorvia create api --template fedora-40 --cpus 4 --memory 8Gi --disk-size 40Gi
```

**Blueprints** — multi-VM stacks (profiles applied inside the blueprint):

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

**Templates** — 43 named keys (aliases included): Ubuntu, Fedora, CentOS Stream, Debian, RHEL, Alma, Rocky, OpenSUSE, Alpine, Arch, Oracle, FreeBSD, Flatcar, Talos, Windows.

```bash
zorvia templates
zorvia template ubuntu-24.04
```

Catalog: [docs/OS_TEMPLATES.md](docs/OS_TEMPLATES.md)

---

## Operator toolkit

```bash
zorvia drift desired.yaml
zorvia drift desired.yaml --actual live.yaml --fail-on high -o json
zorvia plan desired.yaml --vm payments-01 --fail-on-downtime
zorvia guest-insight payments-01 --strict
zorvia image-bundle --distro ubuntu --version 22.04 --storage-class fast
zorvia terraform-scaffold --output ./terraform/zorvia-vm --url https://HOST:30152
zorvia inventory
zorvia activity
zorvia maintenance-plan worker-3
zorvia placement-advisor --cpu 2 --memory-gib 4
```

| Topic | Doc |
|-------|-----|
| Drift | [docs/DRIFT_GUARD.md](docs/DRIFT_GUARD.md) |
| Change plans | [docs/CHANGE_PLANNER.md](docs/CHANGE_PLANNER.md) |
| Guest insight | [docs/GUEST_INSIGHT.md](docs/GUEST_INSIGHT.md) |
| Golden images | [docs/GOLDEN_IMAGES.md](docs/GOLDEN_IMAGES.md) |
| Terraform | [docs/TERRAFORM.md](docs/TERRAFORM.md) |
| Kryton | [docs/KRYTON_INTEGRATION.md](docs/KRYTON_INTEGRATION.md) |
| Inventory | [docs/VCENTER_FEATURE_MATRIX.md](docs/VCENTER_FEATURE_MATRIX.md) |
| Snapshots | [docs/SNAPSHOTS.md](docs/SNAPSHOTS.md) |
| TUI | [docs/INTERACTIVE_TUI.md](docs/INTERACTIVE_TUI.md) |
| Docs index | [docs/README.md](docs/README.md) |
| Command card | [QUICK_REFERENCE.md](QUICK_REFERENCE.md) |

Kryton: set `KRYTON_URL` (+ usually `KRYTON_TOKEN`, `KRYTON_PROJECT`) on the API pod. Tokens never reach the browser.

Terraform: no HashiCorp registry provider binary yet — scaffold + `schema.json` + `terraform/modules/zorvia_vm` ship today.

---

## Config & library

```bash
zorvia validate examples/ubuntu-cloud-init.yaml
zorvia create my-vm --from-file examples/basic-vm.yaml
zorvia generate web --template ubuntu --kubevirt -o web.yaml
zorvia config-init && zorvia config-show
# ~/.config/zorvia/config.toml  ·  /etc/zorvia/config.toml
```

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
```

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
make help
make test
make ci
make release
make tui
zorvia commands
./deploy/remote-deploy.sh <host> sus --quick
```

Architecture notes: [DEVELOPMENT.md](DEVELOPMENT.md).

---

## Security

No `unsafe` on the product path. CORS off unless configured. TLS verification enforced.
Profile/blueprint storage blocks path traversal. API errors are sanitized; auth inputs bounded.

Report privately via [GitHub Security Advisories](https://github.com/zyvorai/zorvia/security/advisories)
or `info@zyvor.dev` — [SECURITY.md](SECURITY.md).

---

## Contributing · License

PRs welcome — [CONTRIBUTING.md](CONTRIBUTING.md).

Copyright 2026 ZyvorAI Labs Private Limited.  
[Apache License 2.0](http://www.apache.org/licenses/LICENSE-2.0) only
([LICENSE](LICENSE), [NOTICE](NOTICE)) — not dual-licensed with MIT.

Built on [KubeVirt](https://kubevirt.io/) and [kube-rs](https://github.com/kube-rs/kube).
