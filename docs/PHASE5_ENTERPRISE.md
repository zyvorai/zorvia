# Phase 5 — Enterprise capabilities

Tracked in [FEATURE_MATURITY.md](FEATURE_MATURITY.md). All require
`ZORVIA_EXPERIMENTAL=1` **and** the per-capability flag.

| Capability | Env flag | API |
|------------|----------|-----|
| S3 immutable backups | `ZORVIA_FEATURE_S3_BACKUP=1` | `POST /api/v1/enterprise/s3-backup/plan` |
| Cross-cluster DR | `ZORVIA_FEATURE_CROSS_CLUSTER_DR=1` | `POST /api/v1/enterprise/cross-cluster-dr/plan` |
| Transiva VMware migration | `ZORVIA_FEATURE_TRANSIVA=1` | `POST /api/v1/enterprise/transiva/plan` |
| Golden-image pipeline | `ZORVIA_FEATURE_GOLDEN_PIPELINE=1` | `POST /api/v1/enterprise/golden-pipeline/plan` |
| GPU/SR-IOV/NUMA | `ZORVIA_FEATURE_GPU_NUMA=1` | `POST /api/v1/enterprise/placement/gpu-numa` |
| Fleet multi-cluster | `ZORVIA_FEATURE_FLEET=1` | `GET /api/v1/enterprise/fleet` |

Plans are **dry-run / documentation of intent**. They do not upload to S3, talk to
vCenter, or mutate remote clusters yet. Admin RBAC (`cluster.admin`) is required.

## OIDC (Beta)

Secure OIDC is separate from Phase 5 flags:

```bash
export ZORVIA_OIDC_ENABLED=1
export ZORVIA_OIDC_ISSUER=https://idp.example.com
export ZORVIA_OIDC_CLIENT_ID=...
export ZORVIA_OIDC_CLIENT_SECRET=...
export ZORVIA_OIDC_REDIRECT_URI=https://zorvia.example.com/api/v1/auth/oidc/callback
# optional: ZORVIA_OIDC_NAME ZORVIA_OIDC_SCOPES
```

Flow: discovery → PKCE authorize → code exchange → JWKS `id_token` verify
(iss/aud/nonce/exp) → JIT local user (`oidc:<sub>`) → mint Zorvia JWT.
