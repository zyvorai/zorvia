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
| `pod-ops` | Pods: logs, exec, events, YAML, restart/delete | Beta | `cluster.admin` only; `pods` delete + `pods/log` + `pods/exec` RBAC; exec, restart and delete audited — [PODS.md](PODS.md) |
| `rescue-mode` | Rescue mode: hostname/SSH-key/enable-SSH via a Job-mounted disk | Beta | `cluster.admin` only; 3 of 5 UI operations wired (reset-password, install-packages, inspect deferred); needs on-cluster verification of the mounted PVC's disk-image path before trusting it against a production VM — [RESCUE.md](RESCUE.md) |
| `schedulers` | Backup/power/alert/warm-pool | Beta | Lease election; in-process |
| `rook-storage` | Rook-Ceph management | Beta | Bootstrap SA optional |
| `atlas-storage` | Atlas storage integration: inventory, backend/RBD/object-store lifecycle, AI insights, observability, governance, DataBridge | Beta | Optional, gated on `ATLAS_URL`; `storage.admin`; disaster recovery is scaffolding on Atlas's own side — unverified without a 2nd real Ceph cluster — see [ATLAS_INTEGRATION.md](ATLAS_INTEGRATION.md) |
| `helm-chart` | Helm chart | Beta | Lab + hardened production values |
| `oidc` | OIDC / SSO | Beta | `ZORVIA_OIDC_ENABLED=1` + PKCE/JWKS; lab bake-off [OIDC_LAB.md](OIDC_LAB.md) |
| `ai-troubleshoot` | AI troubleshooting | Model only | Rules demo, not ML RCA |
| `dr-replication` | DR replication pairs | Model only | In-memory; needs experimental |
| `incremental-backup` | Incremental backup fields | Model only | Executions still full snapshots |
| `s3-immutable-backup` | Off-cluster S3 backups, restore, recovery drills | Experimental | Verified live end to end on one stack (KubeVirt 1.9, CDI 1.66, Rook-Ceph RBD with Filesystem and Block volumes, Atlas-provisioned RGW bucket): snapshot → temp PVC → agent Job → encrypted upload → read-back, restore with identical SHA-256, and a recovery drill with serial-console boot proof; earlier against SeaweedFS including Object Lock. Not yet covered: Object Lock on RGW, other CSI drivers, guest-agent or application-level drill checks — see [SUPPORT_MATRIX.md](SUPPORT_MATRIX.md) and docs/OFFCLUSTER_BACKUP.md |
| `cross-cluster-dr` | Cross-cluster DR | Experimental | Plan API only |
| `transiva-migration` | Transiva migration | Experimental | Plan API only; superseded by `vm-import-h2kvm` |
| `vm-import-h2kvm` | VMware → KubeVirt import via h2kvm Jobs | Experimental | Admin console, fail-closed preflight, explicit target NIC mapping, guarded cutover, boot policy, data-preserving cancellation; needs `ZORVIA_H2KVM_IMAGE`; not yet exercised against a real vCenter |
| `golden-image-pipeline` | Golden-image pipeline | Experimental | Plan + `POST …/golden-pipeline/run` (CDI apply); scan/sign deferred |
| `gpu-sriov-numa` | GPU/SR-IOV/NUMA | Experimental | Plan API only |
| `fleet-multicluster` | Fleet multi-cluster | Experimental | Live read-only nodes/VM inventory and admin console; explicit enrollment, partial results and freshness; real multi-cluster validation pending; see docs/FLEET.md |

Set `ZORVIA_EXPERIMENTAL=1` only in non-production environments to exercise model-only / experimental surfaces.
See [PHASE5_ENTERPRISE.md](PHASE5_ENTERPRISE.md) for env flags and endpoints.
