# Phase 5 — Enterprise capability scaffold

These capabilities are **experimental / not shipped**. Tracked in
[FEATURE_MATURITY.md](FEATURE_MATURITY.md). Enable demos only with
`ZORVIA_EXPERIMENTAL=1`.

| Capability | Env / flag | Status |
|------------|------------|--------|
| S3 immutable backups | `ZORVIA_FEATURE_S3_BACKUP=1` | Scaffold |
| Cross-cluster DR | `ZORVIA_FEATURE_CROSS_CLUSTER_DR=1` | Scaffold |
| Transiva VMware migration | `ZORVIA_FEATURE_TRANSIVA=1` | Scaffold |
| Golden-image pipeline | `ZORVIA_FEATURE_GOLDEN_PIPELINE=1` | Partial (CDI images exist) |
| GPU/SR-IOV/NUMA | `ZORVIA_FEATURE_GPU_NUMA=1` | Scaffold |
| Fleet multi-cluster | `ZORVIA_FEATURE_FLEET=1` | Models only |

Implementation order after P0/P1 hardening: S3 backups → Transiva → golden pipeline → DR → GPU → fleet.
