# Zorvia

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="web/public/zyvor-logo-on-dark.png">
    <img src="web/public/zyvor-logo.png" alt="Zorvia" width="220">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/zyvorai/zorvia/actions"><img src="https://github.com/zyvorai/zorvia/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.76%2B-orange.svg" alt="Rust"></a>
  <a href="https://kubevirt.io/"><img src="https://img.shields.io/badge/KubeVirt-native-6d28d9.svg" alt="KubeVirt"></a>
</p>

<p align="center"><b>KubeVirt VMs without the YAML tax.</b></p>

<p align="center">
Zorvia is a Rust toolkit for crafting and day-2 managing virtual machines on Kubernetes:<br>
CLI, interactive TUI, and a signed-in web console on the same Fabric-compatible API.
</p>

<p align="center">
  <a href="https://github.com/zyvorai/zorvia">Repository</a> ·
  <a href="https://zyvor.dev">zyvor.dev</a> ·
  Apache-2.0 only ·
  <a href="CHANGELOG.md">Changelog</a>
</p>

<p align="center">
  <img src="docs/client-presentations/screenshots/03-dashboard.png" alt="Zorvia dashboard" width="880">
</p>

**Contents:** [Install](#install) · [Quick start](#quick-start) · [Why Zorvia](#why-zorvia) · [Platform surface](#platform-surface) · [Day-2 commands](#day-2-commands) · [Web console & API](#web-console--api) · [Profiles · blueprints · templates](#profiles--blueprints--templates) · [Operator toolkit](#operator-toolkit) · [Config & library](#config--library) · [Develop](#develop) · [Project security](#project-security) · [Contributing · License](#contributing--license)

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
# remote-deploy.sh fills in ZORVIA_EXPOSE_HOST/HOST automatically; applying
# k8s.yaml directly leaves the __ZORVIA_EXPOSE_HOST__ placeholder in place --
# substitute it yourself first, e.g.: sed -i "s/__ZORVIA_EXPOSE_HOST__/<HOST>/g" deploy/k8s.yaml
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
| Reach the guest | Serial, VNC, in-browser SSH — real pty, password auth works (authenticated WebSockets) |
| Who can do what | Admin/user/viewer accounts (`/app/access-control`), admin-only user management, last-admin-lockout protection |
| Day-2 without downtime | Hotplug CPU/memory/disk/NIC, live migration, disk resize — CLI and web console |
| Catch drift | `zorvia drift` + `zorvia plan` |
| Golden library | quay.io containerdisks + CDI `image-bundle`; web console download now applies real DataVolumes |
| Distributed storage | Rook-Ceph pools/filesystems/object stores + StorageClass provisioning (`/app/storage`) |
| GitOps | Terraform scaffold + module over the Fabric API |
| Windows plane | Full Create VM wizard support via Kryton when `KRYTON_URL` is set (`/app/create`, inventory at `/app/windows`) |

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

## Platform surface

The quickstart above covers the everyday VM lifecycle. Zorvia's CLI also carries ~178 subcommands across
operator-grade surfaces beyond core VM craft. Per [DEVELOPMENT.md](DEVELOPMENT.md)'s own scope note: these are
**advanced surfaces, not every-cluster guarantees** — the always-supported path is create/day-2/snapshots/drift/plan/golden-images/Terraform;
treat the rest as available, real, and worth exploring, not a promise that every module fits every deployment.

**Security & compliance**

```bash
zorvia security-scan prod-db --scan-type deep
zorvia security-harden prod-db --profile cis
zorvia compliance-check prod-db --framework soc2
zorvia audit-list --security-only
```

**Cost & FinOps**

```bash
zorvia cost-analyze prod-db --period 30d
zorvia cost-optimize --high-priority-only
zorvia cost-waste --waste-type idle
zorvia budget-create platform --amount 5000 --period monthly --alert-threshold 80
```

**Multi-tenancy & access**

```bash
zorvia tenants-create platform-team --owner alice --email alice@example.com
zorvia users-create bob --email bob@example.com --role operator
zorvia users-assign-role bob db-admin --scope namespace:staging
zorvia quotas-create platform-quota --namespace platform --preset large
```

**Backup & disaster recovery**

```bash
zorvia backup-create prod-db --backup-type incremental
zorvia backup-schedule-create nightly --schedule daily --vm prod-db
zorvia backup-restore prod-db-full-20260901 --target prod-db-restore --start
zorvia recovery-plan primary-site-failover
```

**High availability**

```bash
zorvia ha-config prod-db --enable --priority critical --eviction-strategy live-migrate
zorvia ha-status prod-db
zorvia evacuate-node worker-3 --max-parallel 4 --plan
```

**Automation & workflows**

```bash
zorvia automation-create nightly-snapshot --trigger schedule --enable
zorvia workflow-create dr-failover --template disaster-recovery
zorvia workflow-run dr-failover --watch
zorvia schedule-create weekly-backup --rule nightly-snapshot --schedule weekly --enable
```

**Observability & insights**

```bash
zorvia metrics-query cpu_usage --aggregation p95
zorvia alerts-active --severity critical
zorvia insights-generate prod-db --insight-type performance
zorvia trends-analyze cpu_usage --window 24
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

<p align="center">
  <img src="docs/client-presentations/screenshots/04-vms-list.png" alt="Zorvia VM list" width="880">
</p>

---

## Web console & API

| Route | Purpose |
|-------|---------|
| `/app` | Dashboard |
| `/app/create` | Linux (cloud-init) or Windows (Kryton golden images) create |
| `/app/vms/:name` | Power, port-forwards, cloud-init, snapshots, hotplug, disk resize |
| `/app/vms/:name/console` | Serial · VNC · SSH |
| `/app/snapshots` | Snapshot browser |
| `/app/migrations` | Live migration |
| `/app/storage` | Rook-Ceph storage management |
| `/app/windows` | Kryton Windows inventory (optional) |
| `/app/access-control` | User & role management (admin-only) |

<p align="center">
  <img src="docs/client-presentations/screenshots/06-vm-console.png" alt="Zorvia in-browser VM console" width="880">
</p>

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

<p align="center">
  <img src="docs/client-presentations/screenshots/07-vm-metrics.png" alt="Zorvia VM metrics with Prometheus overlay" width="880">
</p>

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

<p align="center">
  <img src="docs/client-presentations/screenshots/20-templates-page.png" alt="Zorvia template catalog" width="880">
</p>

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

## Project security

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
