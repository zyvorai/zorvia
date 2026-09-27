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
- `GET|POST /api/v1/atlas/volumes` — `POST` creates a volume
- `POST /api/v1/atlas/volumes/:id/expand` — `{new_size_bytes}`
- `DELETE /api/v1/atlas/volumes/:id[?confirm=true]`
- `GET /api/v1/atlas/jobs` — most-recent 100 (Atlas's own server-side cap, no pagination)
- `GET /api/v1/atlas/jobs/:id`
- `POST /api/v1/atlas/jobs/:id/cancel` — `409` if the job is already terminal (`succeeded`/`failed`)
- `POST /api/v1/atlas/backends` — register a backend row; only `nfs`/`zfs` `backend_type`s get instantiated live, others land `pending`
- `DELETE /api/v1/atlas/backends/:id[?purge=true]` — refused if volumes still reference it unless `purge=true`
- `POST /api/v1/atlas/backends/:id/discover`, `/cordon`, `/uncordon`
- `GET|POST /api/v1/atlas/maintenance` — `POST {paused}` pauses/resumes Atlas's whole job engine (all tenants)
- `GET /api/v1/atlas/maintenance/orphans`, `GET /api/v1/atlas/upgrade/preflight` (report-only, always `200`; `ready`/`blockers` is the real gate)
- `GET /api/v1/atlas/osds`
- `POST /api/v1/atlas/osds/:id/{out,in}`, `POST /api/v1/atlas/osds/:id/reweight?weight=0.0-1.0`
- `GET|POST /api/v1/atlas/rbd-images[?pool=]` — a **separate identity space** (`rbd:<pool>/<image>`) from the `StorageVolume` abstraction the volume routes above use; don't conflate a "volume" with an "RBD image"
- `DELETE /api/v1/atlas/rbd-images/:pool/:image`
- `POST /api/v1/atlas/rbd-images/:pool/:image/{clone,resize,migrate,flatten,qos}`
- `GET|POST /api/v1/atlas/rbd-images/:pool/:image/snapshots`, `POST .../rollback`, `DELETE .../snapshots/:snap`
- `POST /api/v1/atlas/rbd-usage/refresh` — synchronous, recomputes `used_bytes` for every RBD-backed volume

RBD writes are async jobs, same shape as the volume writes, except `rbd-usage/refresh` (synchronous, like backend lifecycle). `migrate` takes `?dest_pool=`; `qos` takes `?iops=&bps=` (at least one required, `0` clears a cap); `resize` takes `{size_bytes, allow_shrink}` in the body (`allow_shrink` guards against accidental data loss, default `false`); `rollback` takes `{name: <snapshot name>}` in the body, same shape as creating a snapshot.

Unlike the volume routes, **backend lifecycle (create/delete/discover/cordon/uncordon/maintenance) is synchronous** — Atlas returns the finished result directly, no job envelope, no polling needed. OSD ops (`out`/`in`/`reweight`) *are* async jobs, same shape as the volume writes.

All three volume-mutating routes return `202` with Atlas's job envelope (`{"job_id", "state":"queued", "resource": {...}, "links": {"job": "/api/atlas/v1/jobs/:id"}}`), not a finished result — every Atlas write is genuinely async. Poll `GET /api/v1/atlas/jobs/:id` with that `job_id` to find out what actually happened; `state` reaches a terminal value (`succeeded`/`failed`) or stays `pending`/`queued`/`running`/`verifying` in between. The web UI (`RookStorage.tsx`'s Atlas section) does exactly this after every create/expand/delete — `pollAtlasJob()` in `web/src/api/atlas.ts` polls every 1.5s up to a 60s timeout and the UI toasts the real outcome, not just "requested". `POST /volumes` accepts `{name, size_bytes, policy?, pool?, owner?, kubernetes?}` and forwards to Atlas's `CreateVolumeRequest`; when called in the context of a VM, populate `owner: {product: "zorvia", resource_type: "vm", resource_id: <vm name>, role: "data_disk"}` (the web UI's "Attribute to VM" field does this) — that's what makes the volume traceable back to Zorvia in Atlas's own inventory, not a cosmetic detail. `DELETE` always passes `confirm=true` from Zorvia's side; Atlas itself decides (based on storage class / protection tier) whether that was actually required, rejecting with a `400` naming the requirement if it was needed and the caller didn't set it — Zorvia doesn't try to duplicate that judgment. All `/api/v1/atlas/*` routes are protected by Zorvia's existing auth middleware; the volume-mutating routes and `jobs/:id/cancel` additionally require the `storage.admin` permission (same permission that gates Rook administration) — note Atlas's own token-level auth is stricter for delete and cancel specifically (`ROLE_ADMIN` vs. `ROLE_OPERATOR` for create/expand), so those two can still be rejected by Atlas even when Zorvia's own RBAC allows the request through.

## Deliberately not proxied yet

This is a read-only inventory integration plus volume create/expand/delete plus job status polling — not full Atlas lifecycle management. Not proxied:

- Job SSE watch (`GET /jobs/:id/watch`) — Zorvia has no established SSE-proxy pattern; polling `GET /jobs/:id` on the existing 10-15s refresh cadence is good enough for now
- Disaster recovery (peers, mirrors, promote/demote, failover)
- DataBridge (cloud-to-edge DB migration)
- Object-store bucket operations
- Tenant policy/quota writes, AI advisor, alerts, audit export

If any of these become a real need, they follow the same pattern as the volume routes here — add the client method, the handler, and (if it's a write) a permission check and audit entry.

## Production recommendations

1. Run Atlas behind a ClusterIP service reachable only from Zorvia and trusted operators.
2. Keep `ATLAS_TOKEN` in a Kubernetes Secret, minted with a `product.service.zorvia`-style role and the correct `tenant_id`.
3. Rotate the Atlas token independently of Zorvia user credentials.
4. Keep `ATLAS_TLS_INSECURE=false` outside isolated labs.
5. `POST /volumes` is genuinely async: a successful call returns `202` with a
   `job_id` (`{"state":"queued", "resource":{"volume_id":..., "pvc":...}, ...}`),
   not a finished volume — Zorvia's response mirrors that `202` as-is and
   doesn't poll the job to completion. Verified live against a local
   `atlas-gateway` (fake Ceph driver): the request is accepted correctly, but
   the job then fails with `"no Kubernetes cluster is attached; cannot run
   the write path"` when Atlas itself has no `ATLAS_KUBECONFIG` configured —
   Atlas needs a real Kubernetes cluster attached (via `ATLAS_KUBECONFIG` or
   in-cluster config) to actually create the PVC, independent of anything on
   Zorvia's side. Confirm your Atlas instance has that before relying on
   volume creation from Zorvia, and don't assume a `202` means the volume
   exists yet. The same applies to expand/delete.
6. Volume delete requires the `ATLAS_TOKEN` to carry `ROLE_ADMIN` on Atlas's
   side (create/expand only need `ROLE_OPERATOR`) — a token scoped to
   operator-level will get a real `403` from Atlas on delete even though
   Zorvia's own RBAC allowed the request through. Verified live: deleting a
   volume Atlas considers production/protected-class also requires
   `?confirm=true`, which Zorvia always sends — Atlas is the one deciding
   whether that was actually needed.
