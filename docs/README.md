# Zorvia documentation

| Doc | Topic |
|-----|--------|
| [WEB_CONSOLE.md](WEB_CONSOLE.md) | SPA, Fabric HTTP API, console/VNC WebSockets, NodePort expose |
| [INNOVATIVE_FEATURES.md](INNOVATIVE_FEATURES.md) | Profiles, blueprints, health, recommendations |
| [OS_TEMPLATES.md](OS_TEMPLATES.md) | 44 OS templates |
| [SNAPSHOTS.md](SNAPSHOTS.md) | Snapshot create/restore/retention |
| [DISK_MANAGEMENT.md](DISK_MANAGEMENT.md) | Disks and volumes |
| [NETWORK_MANAGEMENT.md](NETWORK_MANAGEMENT.md) | Networking |
| [ADVANCED_FEATURES.md](ADVANCED_FEATURES.md) | Advanced CLI capabilities |
| [THEME.md](THEME.md) | CLI/TUI theme |
| [INTERACTIVE_TUI.md](INTERACTIVE_TUI.md) | Interactive TUI guide |
| [INTERACTIVE_TUI_README.md](INTERACTIVE_TUI_README.md) | TUI overview |
| [TUI_FEATURES_DEMO.md](TUI_FEATURES_DEMO.md) | TUI demo walkthrough |
| [client-presentations/](client-presentations/) | HTML decks for client demos |

Also see root [README.md](../README.md), [QUICK_REFERENCE.md](../QUICK_REFERENCE.md), [DEVELOPMENT.md](../DEVELOPMENT.md), [CHANGELOG.md](../CHANGELOG.md), [CONTRIBUTING.md](../CONTRIBUTING.md), [SECURITY.md](../SECURITY.md).

## Lab

```text
https://<HOST>:30152/          # Zorvia UI (NodePort)
https://<HOST>:30152/api/v1/health
./deploy/remote-deploy.sh <host> sus --quick
```
