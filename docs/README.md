# Zorvia documentation

## Core

| Doc | Topic |
|-----|--------|
| [WEB_CONSOLE.md](WEB_CONSOLE.md) | SPA, Fabric HTTP API, console/VNC/SSH WebSockets, NodePort expose, hotplug, migration, Rook-Ceph storage, user & role management |
| [OS_TEMPLATES.md](OS_TEMPLATES.md) | 43 OS templates |
| [INNOVATIVE_FEATURES.md](INNOVATIVE_FEATURES.md) | Profiles, blueprints, health, recommendations |
| [THEME.md](THEME.md) | CLI/TUI theme |

## Day-2 ops

| Doc | Topic |
|-----|--------|
| [SNAPSHOTS.md](SNAPSHOTS.md) | Snapshot create/restore/retention |
| [DISK_MANAGEMENT.md](DISK_MANAGEMENT.md) | Disks and volumes |
| [NETWORK_MANAGEMENT.md](NETWORK_MANAGEMENT.md) | Networking |
| [ADVANCED_FEATURES.md](ADVANCED_FEATURES.md) | Advanced CLI capabilities |
| [GUEST_INSIGHT.md](GUEST_INSIGHT.md) | QEMU Guest Agent readiness (`zorvia guest-insight`) |
| [GOLDEN_IMAGES.md](GOLDEN_IMAGES.md) | quay.io containerdisks + CDI golden (`zorvia image-bundle`) |

## Drift, plan & inventory

| Doc | Topic |
|-----|--------|
| [DRIFT_GUARD.md](DRIFT_GUARD.md) | Semantic desired-vs-live drift (`zorvia drift`) |
| [CHANGE_PLANNER.md](CHANGE_PLANNER.md) | Operational change plan from drift (`zorvia plan`) |
| [VCENTER_FEATURE_MATRIX.md](VCENTER_FEATURE_MATRIX.md) | Inventory, activity, maintenance, placement |

## Integrations

| Doc | Topic |
|-----|--------|
| [KRYTON_INTEGRATION.md](KRYTON_INTEGRATION.md) | Windows via Kryton — Create VM wizard + inventory (`/app/windows`) |
| [TERRAFORM.md](TERRAFORM.md) | Terraform scaffold + Fabric API module |

## TUI

| Doc | Topic |
|-----|--------|
| [INTERACTIVE_TUI.md](INTERACTIVE_TUI.md) | Canonical interactive TUI guide |
| [INTERACTIVE_TUI_README.md](INTERACTIVE_TUI_README.md) | Pointer → interactive TUI |
| [TUI_FEATURES_DEMO.md](TUI_FEATURES_DEMO.md) | Pointer → demo walkthrough notes |

## Presentations

| Doc | Topic |
|-----|--------|
| [client-presentations/](client-presentations/) | HTML decks for client demos |

Also see root [README.md](../README.md), [QUICK_REFERENCE.md](../QUICK_REFERENCE.md), [DEVELOPMENT.md](../DEVELOPMENT.md), [CHANGELOG.md](../CHANGELOG.md), [CONTRIBUTING.md](../CONTRIBUTING.md), [SECURITY.md](../SECURITY.md).

## Lab

```text
https://<HOST>:30152/          # Zorvia UI (NodePort)
https://<HOST>:30152/api/v1/health
./deploy/remote-deploy.sh <host> sus --quick
```
