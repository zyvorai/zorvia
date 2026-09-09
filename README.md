# Zorvia

[![CI](https://github.com/zyvorai/zorvia/workflows/CI/badge.svg)](https://github.com/zyvorai/zorvia/actions)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![KubeVirt](https://img.shields.io/badge/KubeVirt-native-6d28d9.svg)](https://kubevirt.io/)
[![Version](https://img.shields.io/badge/version-0.2.0-informational.svg)](CHANGELOG.md)

```text
  ███████╗ ██████╗ ██████╗ ██╗   ██╗██╗ █████╗
  ╚══███╔╝██╔═══██╗██╔══██╗██║   ██║██║██╔══██╗
    ███╔╝ ██║   ██║██████╔╝██║   ██║██║███████║
   ███╔╝  ██║   ██║██╔══██╗╚██╗ ██╔╝██║██╔══██║
  ███████╗╚██████╔╝██║  ██║ ╚████╔╝ ██║██║  ██║
  ╚══════╝ ╚═════╝ ╚═╝  ╚═╝  ╚═══╝  ╚═╝╚═╝  ╚═╝
```

**KubeVirt VMs without the YAML tax.**

One Rust binary. Three surfaces — CLI, interactive TUI, signed-in web console.
Create from templates, run stacks from blueprints, day-2 with consoles, snapshots,
drift detection, and a Fabric-compatible HTTP API.

[zyvorai/zorvia](https://github.com/zyvorai/zorvia) · [zyvor.dev](https://zyvor.dev) · Apache-2.0 only

---

## 60-second path

```bash
git clone https://github.com/zyvorai/zorvia.git && cd zorvia
cargo install --path . --features web          # web is the default feature

zorvia create demo --template ubuntu-22.04 --profile web --start
zorvia wait-ready demo --timeout 120
zorvia guest-insight demo
```

Ship the lab console (HTTPS NodePort **30152**):

```bash
./deploy/remote-deploy.sh <host> sus --quick
# or: kubectl apply -f deploy/k8s.yaml
open https://<HOST>:30152/app
```

| Port | Where |
|------|--------|
| **30152** | Cluster front door (UI + API) |
| **5151** | In-pod / `zorvia api-serve --port 5151` |
| **8080** | Config-file default if you omit `--port` |

---

## What you get

| | |
|--|--|
| **43 OS templates** | Ubuntu → Windows, plus FreeBSD, Flatcar, Talos |
| **8 profiles** | `minimal` … `high-perf` — size the VM by workload |
| **5 blueprints** | LAMP, 3-tier, k8s-cluster, CI/CD, dev-stack |
| **Web console** | Create, power, serial/VNC/SSH, expose, snapshots |
| **Drift + plan** | `zorvia drift` / `zorvia plan` for desired-vs-live |
| **Golden images** | quay.io containerdisks + CDI `image-bundle` |
| **Terraform** | Scaffold + module over the same Fabric API |
| **Kryton** | Windows plane at `/app/windows` when `KRYTON_URL` is set |

Same VirtualMachines whether you type in a terminal or click in the browser.
No OpenShift required.

---

## Surfaces

```text
                    ┌──────────── CLI ────────────┐
  templates ──────► │  zorvia create / start / …  │
  profiles   ──────► │  zorvia tui --interactive   │──► KubeVirt
  blueprints ──────► │  https://host:30152/app     │     VirtualMachine
                    └──────────── API ────────────┘
                              │
              serial · VNC · SSH · NodePort expose
              snapshot · clone · pause · drift
```

### CLI

```bash
zorvia recommend database
zorvia create prod-db --template ubuntu-22.04 --profile database
zorvia start prod-db && zorvia wait-ready prod-db
zorvia status prod-db --watch

zorvia deploy lamp --prefix myapp --start
zorvia snapshot-create prod-db --name before-upgrade
zorvia clone prod-db staging-db --start
zorvia pause prod-db && zorvia resume prod-db
zorvia health prod-db --detailed
zorvia tui --interactive
```

### Web console

| Route | Purpose |
|-------|---------|
| `/app` | Dashboard |
| `/app/create` | Linux cloud-init or Windows create |
| `/app/vms/:name` | Power, port-forwards, cloud-init, snapshots |
| `/app/vms/:name/console` | Serial · VNC · SSH tabs |
| `/app/snapshots` | Snapshot browser |
| `/app/windows` | Kryton Windows inventory (optional) |

```text
wss://<HOST>:30152/ws/console/<vm>?token=<jwt>
wss://<HOST>:30152/ws/vnc/<vm>?token=<jwt>
wss://<HOST>:30152/ws/ssh/<vm>?token=<jwt>&user=ubuntu
```

Guest **SSH / VNC / RDP** also expose as NodePort Services — set `ZORVIA_EXPOSE_HOST` for connection hints in the UI. Full API map: [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md).

### HTTP API

```bash
TOKEN=$(curl -sk -X POST https://HOST:30152/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"Admin@321"}' | jq -r .token)

curl -sk -H "Authorization: Bearer $TOKEN" https://HOST:30152/api/vms
zorvia api-serve --tls --port 5151   # local / in-pod
```

Lab bootstrap user is for labs only — change credentials before anything shared.

---

## Catalogs

<details>
<summary><strong>Profiles</strong> (8)</summary>

| Profile | CPU | Memory | Disk | Best for |
|---------|-----|--------|------|----------|
| minimal | 1 | 512Mi | 5Gi | Agents, jump hosts |
| dev | 1 | 2Gi | 10Gi | Local / learning |
| test | 2 | 4Gi | 20Gi | CI |
| web | 4 | 8Gi | 40Gi | Nginx, static |
| prod | 4 | 8Gi | 40Gi | Production apps |
| database | 6 | 16Gi | 200Gi | Postgres, MySQL, Mongo |
| microservice | 2 | 4Gi | 20Gi | Node / container roles |
| high-perf | 8 | 16Gi | 100Gi | ML, heavy I/O |

```bash
zorvia profiles && zorvia profile database
```

</details>

<details>
<summary><strong>Blueprints</strong> (5)</summary>

| Blueprint | VMs | Stack |
|-----------|-----|--------|
| lamp | 2 | MySQL + Apache |
| k8s-cluster | 3 | Control plane + workers |
| 3tier | 3 | DB + app + Nginx |
| cicd | 3 | GitLab + Jenkins + registry |
| dev-stack | 3 | DB + Redis + workspace |

```bash
zorvia blueprints
zorvia deploy lamp --prefix demo --dry-run
zorvia deploy lamp --prefix demo --start
```

</details>

<details>
<summary><strong>Templates</strong> (43 named keys)</summary>

Ubuntu, Fedora, CentOS Stream, Debian, RHEL, Alma, Rocky, OpenSUSE, Alpine, Arch,
Oracle, FreeBSD, Flatcar, Talos, Windows — including version aliases
(`ubuntu` → 22.04, `fedora` → 41, `windows` → 2022, …).

```bash
zorvia templates
zorvia template ubuntu-24.04
zorvia create web1 --template almalinux-9 --cpus 4 --memory 8Gi
```

Full catalog: [docs/OS_TEMPLATES.md](docs/OS_TEMPLATES.md)

</details>

---

## Operator toolkit

```bash
# Desired vs live (or file vs file) — CI-friendly severity gates
zorvia drift desired.yaml
zorvia drift desired.yaml --actual live.yaml --fail-on high -o json

# Conservative execution plan from drift (online / restart / recreate / review)
zorvia plan desired.yaml --vm payments-01
zorvia plan desired.yaml --actual live.yaml --fail-on-downtime

# QEMU Guest Agent readiness without logging in
zorvia guest-insight payments-01 --strict

# CDI golden library
zorvia image-bundle --distro ubuntu --version 22.04 --storage-class fast

# Terraform over Fabric API (no registry provider binary yet)
zorvia terraform-scaffold --output ./terraform/zorvia-vm --url https://HOST:30152

# vCenter-shaped inventory / maintenance / placement
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
| Kryton Windows | [docs/KRYTON_INTEGRATION.md](docs/KRYTON_INTEGRATION.md) |
| Inventory matrix | [docs/VCENTER_FEATURE_MATRIX.md](docs/VCENTER_FEATURE_MATRIX.md) |

---

## Config, YAML, and the Rust crate

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
make help | make test | make ci | make release | make tui
zorvia commands
./deploy/remote-deploy.sh <host> sus --quick
```

Architecture and deeper CLI libraries: [DEVELOPMENT.md](DEVELOPMENT.md).
Command card: [QUICK_REFERENCE.md](QUICK_REFERENCE.md). Full index: [docs/README.md](docs/README.md).

**Near-term:** published Hashicorp registry provider (scaffold + `schema.json` ship today).

---

## Security

No `unsafe` in the product path. CORS off unless configured. TLS verification enforced.
Profile/blueprint storage blocks path traversal. HTTP errors are sanitized. Auth inputs bounded.

Report privately via [GitHub Security Advisories](https://github.com/zyvorai/zorvia/security/advisories)
or `info@zyvor.dev` — [SECURITY.md](SECURITY.md).

---

## Contributing · License

PRs welcome — [CONTRIBUTING.md](CONTRIBUTING.md).

Copyright 2026 ZyvorAI Labs Private Limited.  
Licensed under the [Apache License 2.0](http://www.apache.org/licenses/LICENSE-2.0) only
([LICENSE](LICENSE), [NOTICE](NOTICE)) — not dual-licensed with MIT.

Built on [KubeVirt](https://kubevirt.io/) and [kube-rs](https://github.com/kube-rs/kube).
