# Zorvia documentation

Start here: root [README.md](../README.md) (hero, gallery, quick start) · live site [zyvorai.github.io/zorvia](https://zyvorai.github.io/zorvia/).

## Core

| Doc | Topic |
|-----|--------|
| [LAB.md](LAB.md) | Remote deploy, auth Secret, NodePort **30152**, `lab-smoke.sh` |
| [WEB_CONSOLE.md](WEB_CONSOLE.md) | SPA, Fabric HTTP API, console/VNC/SSH WebSockets, hotplug, migration, Rook, RBAC, audit export |
| [OIDC.md](OIDC.md) | Opt-in OIDC SSO (PKCE + token exchange + JWKS) |
| [OIDC_LAB.md](OIDC_LAB.md) | Lab IdP bake-off (Dex/Keycloak) |
| [FEATURE_MATURITY.md](FEATURE_MATURITY.md) | GA / Beta / Experimental / Model-only (`GET /api/v1/features`) |
| [PHASE5_ENTERPRISE.md](PHASE5_ENTERPRISE.md) | Enterprise plan APIs (S3, Transiva, DR, GPU, fleet) |
| [UPGRADE.md](UPGRADE.md) | 0.3.2 → 0.3.3+ operator notes |
| [OS_TEMPLATES.md](OS_TEMPLATES.md) | 43 OS templates |
| [INNOVATIVE_FEATURES.md](INNOVATIVE_FEATURES.md) | Profiles, blueprints, health, recommendations |
| [THEME.md](THEME.md) | CLI/TUI theme |

## Day-2 ops

| Doc | Topic |
|-----|--------|
| [SNAPSHOTS.md](SNAPSHOTS.md) | Snapshot create/restore/retention |
| [DISK_MANAGEMENT.md](DISK_MANAGEMENT.md) | Disks and volumes |
| [NETWORK_MANAGEMENT.md](NETWORK_MANAGEMENT.md) | Networking |
| [ADVANCED_FEATURES.md](ADVANCED_FEATURES.md) | Platform `zorvia status` + advanced CLI |
| [GUEST_INSIGHT.md](GUEST_INSIGHT.md) | QEMU Guest Agent readiness |
| [GOLDEN_IMAGES.md](GOLDEN_IMAGES.md) | quay.io containerdisks + CDI `image-bundle` |

## Drift, plan & inventory

| Doc | Topic |
|-----|--------|
| [DRIFT_GUARD.md](DRIFT_GUARD.md) | Semantic desired-vs-live drift (`zorvia drift`) |
| [CHANGE_PLANNER.md](CHANGE_PLANNER.md) | Operational change plan (`zorvia plan`) |
| [VCENTER_FEATURE_MATRIX.md](VCENTER_FEATURE_MATRIX.md) | Inventory, activity, maintenance, placement |

## Integrations

| Doc | Topic |
|-----|--------|
| [KRYTON_INTEGRATION.md](KRYTON_INTEGRATION.md) | Windows via Kryton — Create VM wizard + `/app/windows` |
| [TERRAFORM.md](TERRAFORM.md) | Terraform scaffold + Fabric API module |

## TUI

| Doc | Topic |
|-----|--------|
| [INTERACTIVE_TUI.md](INTERACTIVE_TUI.md) | Canonical interactive TUI guide |
| [INTERACTIVE_TUI_README.md](INTERACTIVE_TUI_README.md) | Pointer → interactive TUI |
| [TUI_FEATURES_DEMO.md](TUI_FEATURES_DEMO.md) | Demo walkthrough notes |

## Presentations & social

| Doc | Topic |
|-----|--------|
| [client-presentations/](client-presentations/) | HTML decks for client demos |
| [social/](social/README.md) | Share / OG card (1200×630) and LinkedIn/X card (1600×900) |

Also: [QUICK_REFERENCE.md](../QUICK_REFERENCE.md), [DEVELOPMENT.md](../DEVELOPMENT.md), [CHANGELOG.md](../CHANGELOG.md), [CONTRIBUTING.md](../CONTRIBUTING.md), [SECURITY.md](../SECURITY.md).

## Lab at a glance

```text
https://<HOST>:30152/                 # UI (NodePort)
https://<HOST>:30152/api/v1/health
https://<HOST>:30152/api/v1/features
./scripts/deploy-remote.sh <host> sus --quick
ZORVIA_E2E_PASSWORD=… ./scripts/lab-smoke.sh
```

Full steps: [LAB.md](LAB.md).
