# Zorvia

[![CI](https://github.com/zyvorai/zorvia/actions/workflows/ci.yml/badge.svg)](https://github.com/zyvorai/zorvia/actions)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/github/v/release/zyvorai/zorvia?label=version&color=informational)](CHANGELOG.md)
[![KubeVirt-native](https://img.shields.io/badge/KubeVirt-native-6d28d9.svg)](https://kubevirt.io/)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org/)

![Zorvia — KubeVirt VM platform](docs/social/zorvia-share-card.png)

**Kubernetes VMs, run like a platform — not a pile of YAML.**

**[Read the full docs](https://zyvorai.github.io/zorvia/)** — web console, day-2 ops, OIDC, Helm, and feature maturity.

KubeVirt gives you `VirtualMachine` objects. Zorvia gives you the control plane around them: **create** from a template or blueprint instead of hand-writing CRDs, **operate** day-2 — hotplug, live migrate, snapshot — without downtime, **reach** the guest over an authenticated console/VNC/SSH, **govern** with real RBAC and quotas, and **prove** it after the fact with an exportable audit trail. CLI, interactive TUI, and a signed-in web console — one API, live cluster objects, not a parallel mock store.

[Try it now](#quick-start) · [Watch it work](#console-gallery) · [Talk to sales](mailto:sales@zyvor.dev) · [Star on GitHub](https://github.com/zyvorai/zorvia) · [Changelog](CHANGELOG.md)

## What you get

| Step | Surface | What it does |
|------|---------|--------------|
| **1 · Create** | Templates · profiles · blueprints | 43 OS templates, workload profiles, multi-VM stacks — no hand-written VM CRDs |
| **2 · Day-2** | Hotplug · migrate · snapshot | CPU/memory/disk/NIC hotplug, live migration, snapshots, disk resize |
| **3 · Access** | Console · VNC · SSH | Authenticated WebSockets into the guest from the signed-in console |
| **4 · Guard** | RBAC · quotas · NetworkPolicy | Server-side roles, `ResourceQuota`, `NetworkPolicy` — not client-only checks |
| **5 · Audit** | Trail · JSONL export | Persistent audit DB + `GET /api/audit/export` for SIEM shippers |

![Zorvia dashboard](docs/screenshots/readme-dashboard.png)

## Contents

- [What you get](#what-you-get)
- [Console gallery](#console-gallery)
- [Install](#install)
- [Quick start](#quick-start)
- [Why teams pick Zorvia](#why-teams-pick-zorvia)
- [What's inside](#whats-inside)
- [How it stacks up](#how-it-stacks-up)
- [Platform surface](#platform-surface)
- [Day-2 commands](#day-2-commands)
- [Web console & API](#web-console--api)
- [Profiles · blueprints · templates](#profiles--blueprints--templates)
- [Operator toolkit](#operator-toolkit)
- [Config & library](#config--library)
- [Develop](#develop)
- [Roadmap](#roadmap)
- [Project security](#project-security)
- [Get involved](#get-involved)
- [Lab deploy](docs/LAB.md) · [Pods: logs & exec](docs/PODS.md) · [OIDC lab](docs/OIDC_LAB.md) · [Social assets](docs/social/README.md)

## Console gallery

One continuous session on a real lab cluster (HTTPS NodePort **30152**) — dashboard → VM list → in-browser console → live metrics. Not mockups.

![Zorvia demo](docs/screenshots/readme-demo.gif)

**Dashboard → VM list → open a VM's console → watch a real guest boot.** ~15–20s, no audio.

![VM list](docs/screenshots/readme-vms-list.png)
*Every `VirtualMachine`, real status, real names — not sample data.*

![In-browser console](docs/screenshots/readme-vm-console.png)
*Serial console over an authenticated WebSocket. Real pty, real prompt.*

![VM metrics](docs/screenshots/readme-vm-metrics.png)
*CPU/memory pulled from the cluster, not synthesized for the screenshot.*

![Template catalog](docs/screenshots/readme-templates.png)
*43 named OS templates — the same list `zorvia templates` prints.*

---

## Install

One binary, one Helm chart — bring your own KubeVirt cluster.

```bash
git clone https://github.com/zyvorai/zorvia.git
cd zorvia
cargo install --path . --features web   # `web` is the default feature
# or: cargo build --release --features web → target/release/zorvia
```

Requires Rust **1.89+**, a kubeconfig, and a cluster with **KubeVirt** (CDI optional for golden images / CDI clone).

**Helm (cluster install):**

```bash
helm upgrade --install zorvia charts/zorvia -n zorvia-system --create-namespace \
  -f charts/zorvia/values-lab.yaml
```

Production values: `charts/zorvia/values-production.yaml`. Lab remote deploy: [docs/LAB.md](docs/LAB.md).

---

## Quick start

Profile → create → start → reach the guest. Five commands, one real VM.

```bash
# Size from a profile, then create
zorvia profile database
zorvia create prod-db \
  --template ubuntu-22.04 \
  --cpus 6 --memory 16Gi --disk-size 200Gi
zorvia start prod-db
zorvia wait-ready prod-db --timeout 120
zorvia guest-insight prod-db
zorvia status                 # platform components + features
zorvia status prod-db --watch
```

Multi-VM stack:

```bash
zorvia blueprints
zorvia deploy lamp --prefix myapp --start
zorvia list
```

Web console (HTTPS NodePort **30152**, self-signed — use `curl -sk`):

```bash
./scripts/deploy-remote.sh <host> sus --quick
# or: make deploy-remote H=<host> U=sus ARGS=--quick
open https://<HOST>:30152/app
```

| Port | Role |
|------|------|
| **30152** | Cluster front door (UI + API) |
| **5151** | In-pod listen / `zorvia api-serve --tls --port 5151` |
| **8080** | Config-file default when `--port` is omitted |

```bash
# Health + login + list VMs
curl -sk https://HOST:30152/api/v1/health | jq .
TOKEN=$(curl -sk -X POST https://HOST:30152/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"…"}' | jq -r .token)
curl -sk -H "Authorization: Bearer $TOKEN" https://HOST:30152/api/vms | jq .
curl -sk -H "Authorization: Bearer $TOKEN" \
  'https://HOST:30152/api/audit/export?format=jsonl' | head
```

Lab admin password comes from the `zorvia-auth` Secret (printed once on first deploy) — see [docs/LAB.md](docs/LAB.md). Change credentials before anything shared.

---

## Why teams pick Zorvia

KubeVirt is excellent at running VMs. What's missing is the control plane
around them — who's allowed to create what, how you hotplug and live-migrate
without downtime, how you actually reach the guest, and how you prove
afterward who did what.

Zorvia is that control plane: **one real backend** behind a CLI, a TUI, and a
web console, all driving the same Kubernetes objects underneath —
`ResourceQuota`, `NetworkPolicy`, `VirtualMachineSnapshot`, real Node
capacity, real KubeVirt migration phases. If a page shows a number, it came
from the cluster. If a button says it does something, it does it.

Run the [quickstart](#quick-start) in five minutes, or [talk to us](#get-involved)
about running it in production.

## What's inside

Two dozen capabilities, grouped by the job they do — not one wall of a table.

### Ship it
- **43** named OS templates — Ubuntu, Fedora, CentOS Stream, Debian, RHEL,
  Alma, Rocky, openSUSE, Alpine, Arch, Oracle, FreeBSD, Flatcar, Talos,
  Windows — no hand-written VM CRDs.
- **8** resource profiles sized by workload (`--cpus`/`--memory`/`--disk-size`,
  or apply inside a blueprint).
- **5** blueprints for multi-VM stacks: LAMP, 3-tier, k8s-cluster, CI/CD,
  dev-stack.
- Golden image library: quay.io containerdisks + CDI `image-bundle` — the web
  console's download button applies a real `DataVolume`.
- Windows plane: a Create VM wizard backed by Kryton when `KRYTON_URL` is set
  (`/app/create`, inventory at `/app/windows`).

### Run day-2, without downtime
- Hotplug CPU, memory, disk, and NIC; live migration; disk resize — from the
  CLI or the web console.
- Catch drift before it ships: `zorvia drift` diffs desired vs. live,
  `zorvia plan` gates a change on downtime.
- Browser ops has parity with the CLI: create, power, console, expose, and
  snapshot a VM without leaving `/app`.
- Debug the platform itself, not just the guest: the Pods page shows every
  pod with colorized live logs, an `exec -it` shell, events, and YAML in a
  Terminal.app-style panel. Restart/delete is admin-only and audited.

### Reach the guest, govern who can
- Serial, VNC, and in-browser SSH — a real pty, password auth works, all
  over authenticated WebSockets.
- Admin / user / viewer accounts at `/app/access-control`, admin-only user
  management, and last-admin-lockout protection.
- `ResourceQuota` and `NetworkPolicy` CRUD (`/app/quotas`,
  `/app/network-policies`) — server-side enforcement, not a client-only
  checkbox.
- Opt-in OIDC SSO (Beta) via `ZORVIA_OIDC_ENABLED=1`.

### Prove it, protect it
- Compliance posture checked against actual VM specs — PCI-DSS, HIPAA, SOC2
  (`/app/compliance`).
- Persistent audit trail with JSONL export (`GET /api/audit/export`) for
  whatever SIEM you already run.
- VolumeSnapshots plus scheduled backups (`/app/backups`,
  `/app/backup-scheduler`).
- Automate power state with recurring start/stop/restart (`/app/schedules`),
  and get paged on the way there — alerts and HTTPS webhooks with retry
  (`/app/alerts`, `/app/webhooks`).

### Fit more VMs, spend less
- Placement Advisor and Capacity Planning against real Node capacity
  (`/app/placement`, `/app/capacity`).
- Resource Optimizer right-sizes from real usage, not guesses
  (`/app/optimizer`).
- Zones, Analytics, and a Service Map show what's running where
  (`/app/zones`, `/app/analytics`, `/app/service-map`).
- Warm Pools keep ready-to-claim VMs on standby for burst capacity
  (`/app/warm-pools`).

### Bring your own infrastructure
- Distributed storage: Rook-Ceph pools, filesystems, and object stores, plus
  StorageClass provisioning (`/app/storage`).
- Optional pluggable storage control plane via Atlas (`ATLAS_URL`) — Ceph/NFS/ZFS
  backends, capacity overview, and volume provisioning alongside Rook-Ceph
  (`/app/storage`).
- GitOps: a Terraform scaffold and module over the Fabric API.

No OpenShift tax. Same `VirtualMachine` objects, whether you're at a
terminal or in a browser.

```mermaid
flowchart LR
    subgraph You["You"]
        CLI["CLI\nzorvia create / clone / plan"]
        TUI["Interactive TUI\nzorvia tui"]
        WEB["Web console\nhttps://host:30152/app"]
    end

    subgraph API["Fabric API (same backend for all three)"]
        AUTH["Auth · RBAC · audit"]
        WS["WebSockets: serial · VNC · SSH"]
    end

    subgraph K8S["Your Kubernetes cluster"]
        VM["VirtualMachine / VMI\n(KubeVirt)"]
        SNAP["VirtualMachineSnapshot"]
        NP["NetworkPolicy"]
        RQ["ResourceQuota"]
        PVC["PVC / DataVolume\n(CDI, Rook-Ceph)"]
    end

    CLI --> API
    TUI --> API
    WEB --> API
    API --> VM
    API --> SNAP
    API --> NP
    API --> RQ
    API --> PVC
```

One Fabric API, three front ends, real objects at the other end every time.

## How it stacks up

Comparison, not vibes:

|  | Hand-rolled `kubectl`/`virtctl` | Generic K8s dashboards | OpenShift Virtualization | **Zorvia** |
|---|---|---|---|---|
| VM create/day-2 without hand-written YAML | ❌ | ⚠️ view-only for VMs | ✅ | ✅ |
| Same capability from CLI, TUI, *and* web | ❌ | ❌ (web only) | ⚠️ web + `virtctl`, no TUI | ✅ |
| Cost/right-sizing, compliance, HA built in | ❌ | ❌ | ⚠️ partial add-ons | ✅ real, on by default |
| Drift detection + change-plan gating | ❌ | ❌ | ❌ | ✅ (`zorvia drift` / `zorvia plan`) |
| Runs on any KubeVirt cluster | ✅ | ✅ | ❌ OpenShift only | ✅ |
| Open source, Apache-2.0 | ✅ | varies | ❌ | ✅ |

Zorvia isn't a general Kubernetes dashboard — it's opinionated about one thing: VMs on KubeVirt, done like a platform.

---

## Platform surface

The quickstart above is the everyday path. Underneath it, Zorvia's CLI
carries roughly **178 subcommands** across operator-grade surfaces most teams
grow into over time — security, cost, multi-tenancy, backup/DR, HA,
automation, observability. Per [DEVELOPMENT.md](DEVELOPMENT.md)'s own scope
note, treat these as **advanced surfaces, not every-cluster guarantees**: the
always-supported path is create → day-2 → snapshots → drift → plan → golden
images → Terraform. Everything below is real and shipped — not a promise
that every module fits every deployment.

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

Once something's running, the everyday loop:

```bash
zorvia list
zorvia get prod-db
zorvia status
zorvia status prod-db --watch
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

Same backend, same auth, as the CLI — the console is a signed-in SPA served
from the same NodePort as the API. This is the short version; the full route
map lives in [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md).

| Route | Purpose |
|-------|---------|
| `/app` | Dashboard |
| `/app/create` | Linux (cloud-init) or Windows (Kryton) create |
| `/app/vms/:name` | Power, expose, cloud-init, snapshots, hotplug, resize, Rescue mode (admin) — [docs/RESCUE.md](docs/RESCUE.md) |
| `/app/vms/:name/console` | Serial · VNC · SSH |
| `/app/snapshots` · `/app/migrations` | Snapshots · live migration |
| `/app/storage` · `/app/volumes` | Rook-Ceph · fleet PVCs |
| `/app/access-control` | Users & roles (admin) |
| `/app/pods` | Every pod: Terminal.app-style live logs, shell, events, YAML, restart/delete (admin) — [docs/PODS.md](docs/PODS.md) |
| `/app/disk-images` | Image catalog as a black Terminal.app listing (`zorvia images` on the CLI) |
| `/app/quotas` · `/app/network-policies` | Real `ResourceQuota` / `NetworkPolicy` |
| `/app/templates` · `/app/windows` | OS catalog · Kryton inventory |
| `/app/placement` · `/app/capacity` · `/app/optimizer` | Placement · headroom · right-sizing |
| `/app/backups` · `/app/schedules` · `/app/alerts` | Backup · power cron · alerts |

```text
wss://<HOST>:30152/ws/console/<vm>?token=<jwt>
wss://<HOST>:30152/ws/vnc/<vm>?token=<jwt>
wss://<HOST>:30152/ws/ssh/<vm>?token=<jwt>&user=ubuntu
wss://<HOST>:30152/ws/pods/<ns>/<pod>/logs?token=<jwt>     # cluster.admin
wss://<HOST>:30152/ws/pods/<ns>/<pod>/exec?token=<jwt>     # cluster.admin
```

Set `ZORVIA_EXPOSE_HOST` for correct NodePort SSH/VNC/RDP hostnames in the UI.

**Fabric API (same auth as the console):**

| Method | Path | Notes |
|--------|------|-------|
| POST | `/api/v1/auth/login` | JWT |
| GET | `/api/v1/health` · `/api/v1/features` | Liveness · maturity registry |
| GET/POST | `/api/vms` | List / create |
| POST | `/api/vms/:name/start\|stop\|restart` | Power |
| GET/POST | `/api/vms/:name/snapshots` | Snapshots |
| GET | `/api/audit/export` | JSONL audit trail (`?format=jsonl`) |

Password from `zorvia-auth` — not committed defaults. Lab smoke: [docs/LAB.md](docs/LAB.md).

---

## Profiles · blueprints · templates

Look up a shape, ship a stack, or name an OS — three lookup tables, no CRD
authoring.

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

Catalog: [docs/OS_TEMPLATES.md](docs/OS_TEMPLATES.md) · gallery shot above under [Console gallery](#console-gallery).

---

## Operator toolkit

Commands operators reach for once the fleet is already running — drift,
change plans, guest insight, golden images, Terraform, placement:

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
| OIDC SSO | [docs/OIDC.md](docs/OIDC.md) |
| Feature maturity | [docs/FEATURE_MATURITY.md](docs/FEATURE_MATURITY.md) |
| Enterprise plans | [docs/PHASE5_ENTERPRISE.md](docs/PHASE5_ENTERPRISE.md) |
| Upgrade | [docs/UPGRADE.md](docs/UPGRADE.md) |
| Terraform | [docs/TERRAFORM.md](docs/TERRAFORM.md) |
| Kryton | [docs/KRYTON_INTEGRATION.md](docs/KRYTON_INTEGRATION.md) |
| Atlas storage | [docs/ATLAS_INTEGRATION.md](docs/ATLAS_INTEGRATION.md) |
| Inventory | [docs/VCENTER_FEATURE_MATRIX.md](docs/VCENTER_FEATURE_MATRIX.md) |
| Snapshots | [docs/SNAPSHOTS.md](docs/SNAPSHOTS.md) |
| TUI | [docs/INTERACTIVE_TUI.md](docs/INTERACTIVE_TUI.md) |
| Docs index | [docs/README.md](docs/README.md) |
| Command card | [QUICK_REFERENCE.md](QUICK_REFERENCE.md) |

Kryton: set `KRYTON_URL` (+ usually `KRYTON_TOKEN`, `KRYTON_PROJECT`) on the API pod. Tokens never reach the browser.

Terraform: no HashiCorp registry provider binary yet — scaffold + `schema.json` + `terraform/modules/zorvia_vm` ship today.

---

## Config & library

Zorvia is a library, not just a binary — validate or generate a VM config
from the CLI, or build one programmatically in Rust:

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

Local dev loop, `make`-first:

```bash
make help
make test
make ci
make status
make release
make tui
make deploy-remote H=<host> U=sus
```

Architecture notes: [DEVELOPMENT.md](DEVELOPMENT.md).

---

## Roadmap

The roadmap is a feature-maturity registry, not a marketing slide: see
**[docs/FEATURE_MATURITY.md](docs/FEATURE_MATURITY.md)** for GA / Beta /
Experimental / Model-only. Only **GA** and documented **Beta** paths are
production promises — query `GET /api/v1/features` on a running API for the
live registry.

Capabilities that need genuinely new infrastructure (not yet shipped):

| Area | What's planned | Why it's not here yet |
|------|-----------------|------------------------|
| Disaster recovery | Site failover, cross-site replication | Needs a real DR/replication engine (`dr-replication` is model-only) |
| Certificates & encryption | Cert lifecycle, disk/volume encryption (KMS) | Needs PKI and key-management integration from scratch |
| Image upload / convert | Upload a disk image from your browser, format conversion | Needs a CDI upload-proxy client (TLS, multipart streaming) |
| Autoscaling | Policy-driven automatic VM scaling | Policy engine exists; needs an execution loop against real load |
| Datacenters & resource pools | vCenter-style hierarchical grouping | No equivalent Kubernetes primitive to build on yet |
| Enterprise SSO | OIDC/SAML with PKCE + JWKS | OIDC Beta via `ZORVIA_OIDC_ENABLED=1`; SAML not yet |
| S3 immutable backup / Transiva / GPU-NUMA | Phase 5 plan APIs | [docs/PHASE5_ENTERPRISE.md](docs/PHASE5_ENTERPRISE.md) |

If one of these is a blocker for adopting Zorvia in your environment, that's
exactly the kind of thing worth a conversation — [reach out](#get-involved)
and tell us what you need.

---

## Project security

No `unsafe` on the product path. CORS off unless configured. TLS verification enforced.
Profile/blueprint storage blocks path traversal. API errors are sanitized; auth inputs bounded.

Report privately via [GitHub Security Advisories](https://github.com/zyvorai/zorvia/security/advisories)
or `info@zyvor.dev` — [SECURITY.md](SECURITY.md).

---

## Get involved

### Running this in production?

Production support, SLAs, and Zyvor Enterprise products are licensed
separately. Contact [sales@zyvor.dev](mailto:sales@zyvor.dev) or see
[zyvor.dev](https://zyvor.dev) — tell us your cluster size and what's on your
list from the [Roadmap](#roadmap) above, and we'll tell you what's already
possible today.

### Evaluating it?

Clone it, run the [quickstart](#quick-start), and see for yourself — every
claim in this README maps to a route or command you can hit right now.
Questions, bug reports, and feature requests are welcome as
[GitHub issues](https://github.com/zyvorai/zorvia/issues); if it's useful to
you, a [star on the repo](https://github.com/zyvorai/zorvia) helps others
find it.

### Contributing code?

PRs welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).

### Open source (Apache-2.0)

This repository is licensed under the [Apache License, Version 2.0](LICENSE).
You may use, modify, and run it for personal, lab, and commercial production
use at no charge, subject to Apache-2.0 (preserve notices / NOTICE where required).
See [NOTICE](NOTICE) — Apache-2.0 only (not dual-licensed with MIT).

Built on [KubeVirt](https://kubevirt.io/) and [kube-rs](https://github.com/kube-rs/kube).
