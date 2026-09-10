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
| `/app/vms/:name/console` | Serial + VNC + SSH tabs |
| `/app/create` | Create VM wizard (Linux or Windows/Kryton) |
| `/app/favorites` | Favorites |
| `/app/snapshots` | Snapshots |
| `/app/migrations` | Live migration: start, list, cancel |
| `/app/storage` | Rook-Ceph: cluster status, pools, filesystems, object stores, StorageClass/VolumeSnapshotClass creation |
| `/app/windows` | Kryton Windows machine inventory (when `KRYTON_URL` is set) |

Auth is JWT (login) or API key. WebSockets pass `?token=` because browsers cannot set `Authorization` on upgrades.

## Create VM

Wizard supports:

- **Guest OS**: Linux | Windows
- **Linux cloud-init**: hostname, username, password, SSH authorized keys, optional `#cloud-config` YAML
- **Windows**: routed entirely through Kryton — picks a golden image from the live Kryton catalog and calls `POST /api/v1/kryton/machines` instead of `/api/vms`; no cloud-init, no KubeVirt-specific fields apply. Requires `KRYTON_URL` (see [KRYTON_INTEGRATION.md](KRYTON_INTEGRATION.md)).
- **Images**: blank disks, PVC name (`pvc:…`), containerdisk refs (`quay.io/…`), or a downloaded golden image (`datavolume:…`, see below)
- **Advanced options** (Linux, optional/collapsible): firmware (BIOS/UEFI/secure boot), CPU model + dedicated placement + isolate-emulator-thread, memory hugepages, machine type, TPM/RNG devices, HyperV/ACPI/APIC feature toggles, multiple disks and NICs
- **Expose**: SSH (22), VNC (5900, Windows create path), RDP (3389) as Kubernetes **NodePort** Services
- **Auto-start** after create (default)

Catalog (`GET /api/images`) returns blank sizes plus major Linux containerdisks. `GET /api/images/cloud` lists unique containerdisk images sourced from the 43 OS templates. `POST /api/images/cloud/download` genuinely imports the image — it applies a real CDI `DataVolume` + stable `DataSource` to the cluster (same manifests as `zorvia image-bundle`, see [GOLDEN_IMAGES.md](GOLDEN_IMAGES.md)) and returns a `datavolume:<name>` reference usable directly as a Create VM disk image.

## Day-2 operations

| Op | How |
|----|-----|
| Start / stop / restart / delete | VM details, list, API |
| Serial console | `/app/vms/:name/console` → Terminal → `GET /ws/console/:name` → KubeVirt `vmis/console` |
| VNC | Console page VNC tab → `GET /ws/vnc/:name` → KubeVirt `vmis/vnc` |
| In-browser SSH | Console page SSH tab → `GET /ws/ssh/:name?user=` → proxies `ssh` or `virtctl ssh` |
| Expose SSH/VNC/RDP | Port-forwards section or create-time flags → NodePort Service labeled `zorvia.io/vm=<name>` |
| Cloud-init update | `POST /api/vms/:name/cloud-init` (annotates VM; volume rewrite requires recreate) |
| Clone | `POST /api/vms/:name/clone` |
| Snapshots | Create / list / delete / revert via Fabric `/api/vms/:name/snapshots…` |
| Hotplug CPU / memory | VM details → Hotplug tab → `POST /api/vms/:name/hotplug/cpu\|memory` |
| Hotplug disk / NIC | VM details → Hotplug tab → `POST/DELETE /api/vms/:name/hotplug/disk\|nic[/:device_id]` |
| Disk resize | VM details → Disks tab → `POST /api/vms/:name/disks/:disk_name/resize` (PVC-backed disks only, grow-only) |
| Live migration | `/app/migrations` → `POST /api/vms/:name/migrate`, list/cancel via `/api/vms/:name/migrations`, `/api/migrations/:id/cancel` |
| Rook-Ceph storage | `/app/storage` → bootstrap operator, create/delete pools/filesystems/object stores, provision StorageClass/VolumeSnapshotClass |

CPU/memory hotplug requires the VM to have been created with `cpu.maxSockets` / `memory.maxGuest` headroom (Fabric create auto-defaults these to 4x sockets / 2x memory unless overridden) **and** the cluster's KubeVirt CR needs `VMLiveUpdateFeatures` in `featureGates` plus `workloadUpdateStrategy.workloadUpdateMethods: ["LiveMigrate"]` and `configuration.liveUpdateConfiguration` set — otherwise KubeVirt accepts the patch but only marks the VM `RestartRequired` instead of live-applying it. Disk hotplug requires KubeVirt's `HotplugVolumes` feature gate; disks must use the `scsi` bus (KubeVirt's admission webhook rejects any other bus for a hotplugged disk — Zorvia defaults to `scsi` when unset). NIC hotplug requires `HotplugNICs` + Multus and reports `501 UNSUPPORTED` when unavailable.

Pause / resume call KubeVirt `virtualmachineinstances/pause` and `…/unpause`. The VMI must be Running. Linux create-time `expose_vnc` opens guest TCP 5900 the same way as Windows.

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
| GET | `/vms/:name/metrics\|logs` | Guest-agent / Prometheus samples + rolling `history` for the graph |
| GET | `/vms/:name/guest-insight` | OS/kernel/interfaces + readiness score |
| POST | `/datavolumes/:name/wait` | Block until CDI DataVolume Succeeded |
| POST | `/vms/:name/wait-ready` | Block until VMI Ready + guest IP |
| GET | `/readyz` | Kube connectivity probe (readiness) |
| GET | `/metrics` | Prometheus text for the API process |
| POST | `/vms/:name/pause\|resume` | KubeVirt VMI pause / unpause (204) |
| GET | `/images/cloud` | Template-backed cloud images (`name`, `distro`, `url`, …) |
| POST | `/images/cloud/download` | Import a golden image: applies a real CDI DataVolume + DataSource, returns `datavolume:<name>` |
| GET | `/images/downloads` | Download job list |
| POST | `/vms/:name/hotplug/cpu` | Set target vCPU count (`count`) |
| POST | `/vms/:name/hotplug/memory` | Grow guest memory by `size_mb` |
| POST/DELETE | `/vms/:name/hotplug/disk[/:device_id]` | Attach / detach a PVC or DataVolume (`path`, optional `bus`, default `scsi`) |
| POST/DELETE | `/vms/:name/hotplug/nic[/:device_id]` | Attach / detach a Multus network (`bridge`) |
| GET | `/vms/:name/disks` | List disks (name, bus, source, resizable) |
| POST | `/vms/:name/disks/:disk_name/resize` | Grow a PVC-backed disk (`size`); rejects shrink and non-PVC disks |
| GET | `/vms/:name/interfaces` | List network interfaces (spec + live MAC/IP from VMI) |
| POST | `/vms/:name/migrate` | Start a live migration (`VirtualMachineInstanceMigration`) |
| GET | `/vms/:name/migrations` | List migrations for a VM |
| GET/POST | `/migrations/:id`, `/migrations/:id/cancel` | Get / cancel a migration |
| POST | `/vms/:name/drift`, `/vms/:name/plan` | Drift Guard / Change Planner against a live VM |
| POST | `/storage/rook/bootstrap` | Install the Rook operator (pinned manifests) |
| GET/POST/DELETE | `/storage/rook/cluster` | CephCluster status / bootstrap / delete |
| GET/POST | `/storage/rook/pools`, DELETE `/storage/rook/pools/:name` | CephBlockPool CRUD |
| GET/POST | `/storage/rook/filesystems`, DELETE `/storage/rook/filesystems/:name` | CephFilesystem CRUD |
| GET/POST | `/storage/rook/objectstores`, DELETE `/storage/rook/objectstores/:name` | CephObjectStore CRUD |
| POST | `/storage/rook/storage-classes` | Provision an RBD or CephFS StorageClass |
| POST | `/storage/rook/volume-snapshot-classes` | Provision a VolumeSnapshotClass |

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

- `kubevirt.io` VMs / VMIs / VirtualMachineInstanceMigrations
- `subresources.kubevirt.io` `virtualmachineinstances/console`, `…/vnc`, `…/pause`, `…/unpause`, `…/addvolume`, `…/removevolume`, `…/addinterface`, `…/removeinterface`
- core `services` (NodePort expose), `serviceaccounts`, `configmaps`
- `storage.k8s.io` storageclasses (write), `snapshot.storage.k8s.io`
- `ceph.rook.io`, `apiextensions.k8s.io`, `rbac.authorization.k8s.io`, `apps`, `scheduling.k8s.io` (Rook operator bootstrap)
- snapshots CRDs, PVCs, pods, events (as in `deploy/k8s.yaml`)

The Rook-bootstrap grants above are intentionally broad — installing an operator means creating its own RBAC, CRDs and Deployments. Skip `POST /storage/rook/bootstrap` and trim those rules if Rook is managed outside Zorvia.

## Limitations

- Blank-disk VMs have no guest OS — console may connect with little/no serial output; use a containerdisk image for real SSH/VNC guest tests.
- Linux create-time `expose_vnc` now creates a NodePort on guest 5900 (same as Windows).
- Clone prefers a CDI DataVolume from the source PVC (`clone_mode=cdi`); falls back to an empty PVC if CDI is missing.
- In-browser SSH needs `ssh` or `virtctl` on the API pod and a running guest with an IP or virtctl access.
- Metrics include KubeVirt phase / paused / node. Guest CPU counters still require virt-launcher metrics.
- Logs pull recent virt-launcher pod lines when the launcher pod is labeled `kubevirt.io/vm=<name>`.
- CPU/memory hotplug depends on KubeVirt version + cluster configuration (see the note under Day-2 operations) — Zorvia's API and RBAC are cluster-agnostic, but without the matching KubeVirt CR settings the request succeeds (spec patched, limits enforced) while KubeVirt itself only marks the VM `RestartRequired` rather than live-applying it. Verify on your cluster before relying on it.
- Disk resize only sees disks declared in the VM's spec at create time; a disk attached via hotplug isn't visible to resize until it's persisted into the spec (or the VM is recreated with it declared).
- **RDP has no in-browser proxy** — unlike SSH/VNC, there's no `/ws/rdp` WebSocket gateway. `expose_rdp` (create-time) and a manual port-forward (day-2) both just open a NodePort on guest 3389; the port-forwards table and the Kryton Windows page both offer a "Download .rdp file" action that hands the host:port (and username, for Kryton) to the user's own native RDP client (mstsc, Microsoft Remote Desktop, Remmina, …) instead.
