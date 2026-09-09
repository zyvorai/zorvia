# Zorvia Web Console & HTTP API

The Fabric-compatible SPA (`web/`) and Axum API (`zorvia api-serve`) manage KubeVirt VMs from the browser. Lab front door is Kubernetes in-pod HTTPS on NodePort **30152**.

## Lab access

```text
UI:      https://<HOST>:30152/
Sign-in: https://<HOST>:30152/sign-in
Health:  https://<HOST>:30152/api/v1/health
```

Default lab bootstrap user (change for non-lab): `admin` / `Admin@321`.

Deploy:

```bash
./deploy/remote-deploy.sh <host> sus --quick
# or
kubectl apply -f deploy/k8s.yaml
```

Useful env on the API pod:

| Variable | Purpose |
|----------|---------|
| `ZORVIA_EXPOSE_HOST` / `HOST` | Public host shown for NodePort SSH/VNC hints |
| `ZORVIA_WEB_DIR` | SPA static root (default `/usr/share/zorvia/web`) |
| `ZORVIA_JWT_SECRET` | JWT signing secret |
| `ZORVIA_ADMIN_USER` / `ZORVIA_ADMIN_PASSWORD` | Bootstrap admin |

## SPA routes (Core)

| Path | Page |
|------|------|
| `/app` | Dashboard |
| `/app/vms` | VM list |
| `/app/vms/:name` | VM details (power, network/port-forwards, cloud-init, snapshots) |
| `/app/vms/:name/console` | Serial terminal + VNC tabs |
| `/app/create` | Create VM wizard |
| `/app/favorites` | Favorites |
| `/app/snapshots` | Snapshots |

Auth is JWT (login) or API key. WebSockets pass `?token=` because browsers cannot set `Authorization` on upgrades.

## Create VM

Wizard supports:

- **Guest OS**: Linux | Windows
- **Linux cloud-init**: hostname, username, password, SSH authorized keys, optional `#cloud-config` YAML
- **Windows**: no cloud-init; expose VNC / RDP checkboxes
- **Images**: blank disks, PVC name (`pvc:…`), or containerdisk refs (`quay.io/…`)
- **Expose**: SSH (22), VNC (5900, Windows create path), RDP (3389) as Kubernetes **NodePort** Services
- **Auto-start** after create (default)

Catalog (`GET /api/images`) returns blank sizes plus major Linux containerdisks. `GET /api/images/cloud` lists unique containerdisk images sourced from the 44 OS templates. Direct download jobs still return an empty item list with a pointer to the catalog.

## Day-2 operations

| Op | How |
|----|-----|
| Start / stop / restart / delete | VM details, list, API |
| Serial console | `/app/vms/:name/console` → Terminal → `GET /ws/console/:name` → KubeVirt `vmis/console` |
| VNC | Console page VNC tab → `GET /ws/vnc/:name` → KubeVirt `vmis/vnc` |
| Expose SSH/VNC/RDP | Port-forwards section or create-time flags → NodePort Service labeled `zorvia.io/vm=<name>` |
| Cloud-init update | `POST /api/vms/:name/cloud-init` (annotates VM; volume rewrite requires recreate) |
| Clone | `POST /api/vms/:name/clone` |
| Snapshots | Create / list / delete / revert via Fabric `/api/vms/:name/snapshots…` |

Out of scope this pass: in-browser SSH to guest:22, live migrate, hotplug, Fabric host NAT.

Pause / resume call KubeVirt `virtualmachineinstances/pause` and `…/unpause`. The VMI must be Running. Linux create-time `expose_vnc` now opens guest TCP 5900 the same way as Windows.

## Fabric-compatible HTTP (under `/api`)

| Method | Path | Notes |
|--------|------|-------|
| GET/POST | `/vms` | List / create |
| GET/DELETE | `/vms/:name` | Get / delete |
| POST | `/vms/:name/start\|stop\|restart` | Power |
| POST/DELETE | `/vms/:name/port-forwards` | NodePort expose |
| POST | `/vms/:name/cloud-init` | Annotate user-data |
| POST | `/vms/:name/clone` | Clone VM CR (+ blank/container disks) |
| GET/POST | `/vms/:name/snapshots` | Snapshot list / create |
| DELETE | `/vms/:name/snapshots/:id` | Delete snapshot |
| POST | `/vms/:name/snapshots/:id/revert` | Revert |
| GET | `/images` | Image catalog |
| GET | `/vms/:name/metrics\|logs` | Phase/paused/node + virt-launcher log tail |
| POST | `/vms/:name/pause\|resume` | KubeVirt VMI pause / unpause (204) |
| GET | `/images/cloud` | Template-backed cloud images (`name`, `distro`, `url`, …) |
| POST | `/images/cloud/download` | Start a CDI import job |
| GET | `/images/downloads` | Download job list |

Native v1 routes remain under `/api/v1/…` (auth, health, namespaced VM power, snapshots, events).

## WebSockets

```text
wss://<HOST>:30152/ws/console/<vm>?token=<jwt>
wss://<HOST>:30152/ws/vnc/<vm>?token=<jwt>
wss://<HOST>:30152/ws/ssh/<vm>?token=<jwt>&user=ubuntu
```

Server proxies to the apiserver using the in-cluster service account (trusts cluster CA; reads `token_file`). Clients should offer subprotocols `plain.kubevirt.io` (console) and `binary.kubevirt.io` (VNC). VMI must be **Running**.

## RBAC

ClusterRole `zorvia` includes:

- `kubevirt.io` VMs / VMIs
- `subresources.kubevirt.io` `virtualmachineinstances/console`, `…/vnc`, `…/pause`, `…/unpause`
- core `services` (NodePort expose)
- snapshots CRDs, PVCs, pods, events (as in `deploy/k8s.yaml`)

## Limitations

- Blank-disk VMs have no guest OS — console may connect with little/no serial output; use a containerdisk image for real SSH/VNC guest tests.
- Linux create-time `expose_vnc` now creates a NodePort on guest 5900 (same as Windows).
- Clone prefers a CDI DataVolume from the source PVC (`clone_mode=cdi`); falls back to an empty PVC if CDI is missing.
- In-browser SSH needs `ssh` or `virtctl` on the API pod and a running guest with an IP or virtctl access.
- Metrics include KubeVirt phase / paused / node. Guest CPU counters still require virt-launcher metrics.
- Logs pull recent virt-launcher pod lines when the launcher pod is labeled `kubevirt.io/vm=<name>`.
