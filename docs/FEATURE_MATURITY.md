# Feature maturity registry

Levels used by Zorvia (also exposed at `GET /api/v1/features` when the API is running):

| Level | Meaning |
|-------|---------|
| **GA** | Real backend, persistence, RBAC, tests, upgrade path |
| **Beta** | Operational with documented limitations |
| **Experimental** | Behind `ZORVIA_EXPERIMENTAL=1`; no production promise |
| **Model only** | Internal types / demos — not marketed as working |

## Current matrix (0.3.3+)

| ID | Feature | Level | Notes |
|----|---------|-------|-------|
| `vm-lifecycle` | VM create/start/stop/delete | GA | KubeVirt + server RBAC |
| `snapshots-restore` | Snapshots & restore | GA | Empty-disk caveat in docs |
| `live-migration` | Live migration | GA | Needs shared storage |
| `web-console` | Web console + auth | GA | OIDC disabled pending secure impl |
| `audit-trail` | Audit trail | Beta | SQLite; no Loki/OTLP export yet |
| `schedulers` | Backup/power/alert/warm-pool | Beta | Lease election; in-process |
| `rook-storage` | Rook-Ceph management | Beta | Bootstrap SA optional |
| `helm-chart` | Helm chart | Beta | Lab + production values |
| `oidc` | OIDC / SSO | Experimental | Hard-disabled in 0.3.3 |
| `ai-troubleshoot` | AI troubleshooting | Model only | Rules demo, not ML RCA |
| `dr-replication` | DR replication pairs | Model only | In-memory; no data movement |
| `incremental-backup` | Incremental backup fields | Model only | Executions are full snapshots |
| `s3-immutable-backup` | S3 immutable backups | Experimental | Phase 5 |
| `cross-cluster-dr` | Cross-cluster DR | Experimental | Phase 5 |
| `transiva-migration` | Transiva VMware migration | Experimental | Phase 5 scaffold |
| `golden-image-pipeline` | Golden-image pipeline | Experimental | Phase 5 |
| `gpu-sriov-numa` | GPU/SR-IOV/NUMA | Experimental | Phase 5 |
| `fleet-multicluster` | Multi-cluster fleet | Experimental | Phase 5 |

Set `ZORVIA_EXPERIMENTAL=1` only in non-production environments to exercise model-only / experimental surfaces.
