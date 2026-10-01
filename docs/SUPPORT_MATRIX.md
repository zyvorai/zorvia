# Validated combinations and measured results

This page records what has actually been run, on which versions, with what outcome. It is
evidence, not a promise: a combination not listed here has not been validated, and a result
here is a single run on one lab host, not a benchmark. Re-run `tests/e2e/guest.sh` on your own
stack before relying on any number.

## Reference lab (single node)

| Component | Version |
|---|---|
| Zorvia | `main` including PRs #145 to #148 (built and deployed 2026-10-01) |
| Kubernetes | v1.36.4+k3s1, one control-plane/worker node (12 vCPU, 30 GiB) |
| OS / kernel / runtime | Ubuntu 26.04.1 LTS, 7.0.0-34, containerd 2.3.4 |
| KubeVirt | v1.9.0 |
| CDI | v1.66.0 |
| Storage | Rook v1.20.8, Ceph 19.2.3 (single OSD, `HEALTH_WARN` as expected for one node), RBD class `zyvor-rbd-prod` |
| Object storage | Ceph RGW bucket provisioned through Atlas |
| Guest image | `quay.io/containerdisks/ubuntu:24.04` |

## Results, 2026-10-01

Produced by `E2E_BACKUP=1 E2E_STORAGE_CLASS=zyvor-rbd-prod tests/e2e/guest.sh` (see
[tests/e2e/README.md](../tests/e2e/README.md)).

| Scenario | Result | Time | Detail |
|---|---|---|---|
| Create, boot, guest ready | PASS | 35 s | Guest has an IP and `Ready=True`; no guest agent in this image |
| Snapshot of a containerdisk guest | PASS | 5 s | Configuration only (no PVC); the API says so in a warning |
| Data snapshot on CSI storage | PASS | 104 s | PVC-backed guest on RBD; `VolumeSnapshot` ready |
| Off-cluster backup | PASS | 95 s | 1 GiB disk, AES-256-GCM, SHA-256, read-back verified |
| Restore from the backup | PASS | 49 s | New VM and PVC created from the manifest |
| Recovery drill | PASS | 216 s | Restore 68 s, boot 131 s, boot proof from the serial console, torn down cleanly |
| API pod restart, guest untouched | PASS | 43 s | `rollout status` to healthy; session still valid; VM still running |
| Live migration under load | SKIP | n/a | Needs two schedulable nodes |

The security checks in [SECURITY.md](../SECURITY.md) were also run live against this build
(permission matrix as Viewer/User/namespace-restricted, socket gating, MFA re-enrolment, logout,
password change, lockout).

## Not validated

- **Live migration**, on any hardware (single-node lab).
- **Node failure and control-plane failover time.** [HA.md](HA.md) describes the supported
  topology; no node-loss run has been recorded, so there is no RTO figure.
- **Windows guests**, including virtio drivers and the guest agent.
- **Guest agent paths**: the lab guest has none, so agent-based drill evidence, agent
  readiness and application-level checks have not been exercised.
- **VMware import** against a real vCenter (and the Migration Cockpit's NIC mapping against
  real Multus networks). Unit and mock tests only. Experimental.
- **Multi-cluster fleet inventory** against real remote clusters. Experimental.
- **Block-mode volumes** (rejected by the backup agent), **other CSI drivers**, **other
  Kubernetes/KubeVirt/CDI versions**, upgrades between Zorvia versions, site-level DR.

## Problems the runs found (and where they stand)

| Finding | Status |
|---|---|
| `POST /vms` waited up to 120 s for the guest but the router cut requests at 30 s, answering 408 and cancelling the rest of the handler | Fixed (wait bounded to 25 s; poll `wait-ready`) |
| Re-enrolling TOTP left 2FA off until the new code was verified | Fixed (pending secret) |
| Deploy script's `kubectl apply` resets `ATLAS_URL`; a stuck `Terminating` API pod keeps the scheduler lease | Operational pitfall; re-set the variable and force-delete the stuck pod |
| On the RBD class CDI chooses Block volume mode and the importer cannot open the device (`Permission denied`) | Workaround: create DataVolumes with `volumeMode: Filesystem`. Not fixed in Zorvia |
| Restored VMs' PVCs outlive the VM | Expected Kubernetes behaviour; delete them yourself |

## Reproduce

```bash
ZORVIA_LAB_HOST=<host> ZORVIA_E2E_PASSWORD=... \
E2E_STORAGE_CLASS=<snapshot-capable class> E2E_BACKUP=1 \
KUBECTL="ssh root@<host> KUBECONFIG=/etc/rancher/k3s/k3s.yaml kubectl" \
tests/e2e/guest.sh
```

`E2E_BACKUP=1` needs the off-cluster target configured on the deployment
([OFFCLUSTER_BACKUP.md](OFFCLUSTER_BACKUP.md)). A scenario that cannot run reports SKIP with the
reason; it never passes silently.
