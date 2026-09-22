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
| `web-console` | Web console + auth | GA | JWT + tokens; OIDC opt-in |
| `audit-trail` | Audit trail | GA | SQLite + `GET /api/audit/export` JSONL; optional `ZORVIA_AUDIT_JSONL` sidecar |
| `schedulers` | Backup/power/alert/warm-pool | Beta | Lease election; in-process |
| `rook-storage` | Rook-Ceph management | Beta | Bootstrap SA optional |
| `helm-chart` | Helm chart | Beta | Lab + hardened production values |
| `oidc` | OIDC / SSO | Beta | `ZORVIA_OIDC_ENABLED=1` + PKCE/JWKS; lab bake-off [OIDC_LAB.md](OIDC_LAB.md) |
| `ai-troubleshoot` | AI troubleshooting | Model only | Rules demo, not ML RCA |
| `dr-replication` | DR replication pairs | Model only | In-memory; needs experimental |
| `incremental-backup` | Incremental backup fields | Model only | Executions still full snapshots |
| `s3-immutable-backup` | S3 immutable backups | Experimental | Plan API only |
| `cross-cluster-dr` | Cross-cluster DR | Experimental | Plan API only |
| `transiva-migration` | Transiva migration | Experimental | Plan API only |
| `golden-image-pipeline` | Golden-image pipeline | Experimental | Plan + `POST …/golden-pipeline/run` (CDI apply); scan/sign deferred |
| `gpu-sriov-numa` | GPU/SR-IOV/NUMA | Experimental | Plan API only |
| `fleet-multicluster` | Fleet multi-cluster | Experimental | Inventory stub |

Set `ZORVIA_EXPERIMENTAL=1` only in non-production environments to exercise model-only / experimental surfaces.
See [PHASE5_ENTERPRISE.md](PHASE5_ENTERPRISE.md) for env flags and endpoints.
