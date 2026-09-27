# Zorvia + Atlas integration

Zorvia stays the KubeVirt VM control plane; Atlas is an optional, separate storage control plane (Ceph/NFS/ZFS today, pluggable backends) that Zorvia can read from — and provision one thing through — for teams who run it instead of (or alongside) managing Rook-Ceph directly.

## Architecture

```text
Browser
  │  Zorvia JWT / API key
  ▼
Zorvia web + API
  │  server-side Atlas bearer token (product.service.* role)
  │  /api/v1/atlas/*
  ▼
Atlas gateway (/api/atlas/v1/*)
  ├─ Ceph driver
  ├─ NFS driver (planned)
  └─ ZFS driver (planned)
```

The browser never receives `ATLAS_TOKEN`. Zorvia owns user authentication; the Atlas service-account token is a server secret, exactly like `KRYTON_TOKEN`.

## Storage page integration

Atlas isn't a separate top-level page — it's a section on the existing Storage page (`/app/storage`, alongside Rook-Ceph), because both are answering the same question ("what storage do I have and what's it doing"), just from two different control planes. If `ATLAS_URL` is unset, the section renders a quiet "not configured" note instead of hiding entirely or erroring.

## Configuration

Set these on the Zorvia API process:

| Variable | Required | Meaning |
|---|---:|---|
| `ATLAS_URL` | yes to enable | Atlas gateway base URL, e.g. `http://atlas-gateway.atlas.svc.cluster.local:8080` |
| `ATLAS_TOKEN` | yes for anything beyond `/health`/`/version` | A pre-minted Atlas JWT with a `product.service.*` role (see auth.rs's `role_level` — that role prefix is specifically for sibling-product service accounts) and the right `tenant_id` claim |
| `ATLAS_TENANT_ID` | yes to create volumes without specifying one per-request | Default tenant scope for volume creation |
| `ATLAS_TIMEOUT_SECS` | no | Upstream HTTP timeout, default 30 |
| `ATLAS_TLS_INSECURE` | no | `true` only for explicitly trusted labs using self-signed TLS |

If `ATLAS_URL` is absent, Zorvia starts normally and the integration reports `enabled=false`. `GET /health` and `GET /version` on the Atlas gateway are unauthenticated; every other route (everything under `/api/atlas/v1/...`) requires the bearer token.

## Zorvia endpoints

- `GET /api/v1/atlas/status` — degrades gracefully to `{"enabled":false}` rather than erroring
- `GET /api/v1/atlas/backends`
- `GET /api/v1/atlas/backends/summary`
- `GET /api/v1/atlas/clusters`
- `GET /api/v1/atlas/clusters/:id/health`
- `GET /api/v1/atlas/pools`
- `GET /api/v1/atlas/ceph/status`
- `GET /api/v1/atlas/ceph/df`
- `GET /api/v1/atlas/storage-classes`
- `GET|POST /api/v1/atlas/volumes` — `POST` is the one write route in this integration

`POST /api/v1/atlas/volumes` accepts `{name, size_bytes, policy?, pool?, owner?, kubernetes?}` and forwards to Atlas's `CreateVolumeRequest`. When called in the context of a VM, populate `owner: {product: "zorvia", resource_type: "vm", resource_id: <vm name>, role: "data_disk"}` (the web UI's "Attribute to VM" field does this) — that's what makes the volume traceable back to Zorvia in Atlas's own inventory, not a cosmetic detail. All `/api/v1/atlas/*` routes are protected by Zorvia's existing auth middleware; `POST /volumes` additionally requires the `storage.admin` permission (same permission that gates Rook administration).

## Deliberately not proxied yet

This is a read-only inventory integration plus one write path (volume create) — not full Atlas lifecycle management. Not proxied:

- Volume **expand**/**delete** (`POST /volumes/:id/expand`, `DELETE /volumes/:id`)
- Backend lifecycle (`POST/DELETE /backends`, discover/cordon/uncordon)
- RBD image clone/resize/QoS
- Disaster recovery (peers, mirrors, promote/demote, failover)
- DataBridge (cloud-to-edge DB migration)
- Object-store bucket operations
- Tenant policy/quota writes, AI advisor, alerts, jobs, audit export

If any of these become a real need, they follow the same pattern as `create_volume` here — add the client method, the handler, and (if it's a write) a permission check and audit entry.

## Production recommendations

1. Run Atlas behind a ClusterIP service reachable only from Zorvia and trusted operators.
2. Keep `ATLAS_TOKEN` in a Kubernetes Secret, minted with a `product.service.zorvia`-style role and the correct `tenant_id`.
3. Rotate the Atlas token independently of Zorvia user credentials.
4. Keep `ATLAS_TLS_INSECURE=false` outside isolated labs.
5. Confirm volume creation actually provisions against your Atlas instance's configured backend before relying on it — Atlas's own write path matures backend-by-backend (Ceph first).
