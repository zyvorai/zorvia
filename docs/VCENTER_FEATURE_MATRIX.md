# Zorvia vCenter-style Operations Feature Matrix

This patch series deliberately uses Kubernetes/KubeVirt primitives rather than cloning VMware APIs.

| Familiar vCenter capability | Zorvia capability in this series | Native representation |
|---|---|---|
| Datacenter / Cluster / Folder inventory | Inventory Navigator | `inventory.zorvia.io/*` labels + live VMI placement |
| Tags | VM Tags | `tag.zorvia.io/<key>` labels |
| Custom attributes | VM Attributes | `attr.zorvia.io/<key>` annotations |
| Recent Tasks / Events / Alarms | Activity Center | Kubernetes Events, normalized and classified |
| Host Maintenance Mode | Maintenance Orchestrator | Node cordon + KubeVirt migration/controlled stop |
| DRS recommendations | Placement Advisor | K8s capacity + labels + KubeVirt VMI requests; recommendation-first |
| vMotion | Existing Zorvia migration | KubeVirt `VirtualMachineInstanceMigration` |
| Snapshots | Existing Zorvia snapshots | KubeVirt snapshots |
| Templates / Content Library | Existing templates + CDI Golden Image patch | KubeVirt/CDI |
| Health / monitoring | Existing Zorvia monitoring and cluster health | Kubernetes/KubeVirt telemetry |
| RBAC | Existing Zorvia multitenancy/RBAC | Kubernetes/Zorvia RBAC |

## Metadata conventions

- `inventory.zorvia.io/datacenter`
- `inventory.zorvia.io/cluster`
- `inventory.zorvia.io/folder`
- `tag.zorvia.io/<tag>`
- `attr.zorvia.io/<attribute>`
- `maintenance.zorvia.io/policy`: `migrate`, `stop`, `skip`, `block`
- `maintenance.zorvia.io/priority`: `0..255`

The maintenance `--force` switch is intentionally explicit. It can stop a workload whose policy otherwise blocks a complete evacuation; it should be controlled by RBAC/change approval in production.
