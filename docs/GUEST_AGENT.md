# In-guest agent (GuestKit / Zyvor guest agent)

An agent inside the guest lets KubeVirt report `agentConnected` and gives Zorvia more than
"the VM is running": online snapshots freeze the guest filesystem around the snapshot
(application-consistent rather than crash-consistent), and drills, readiness checks and
guest inventory can see inside the guest.

## Install at creation

`POST /api/vms` takes `guest_agent`:

| value | installs |
|---|---|
| `zyvor` | The **Zyvor guest agent** from [GuestKit](https://github.com/zyvorai/guestkit): a QEMU-guest-agent-compatible replacement that answers `guest-info`, `guest-get-osinfo`, `guest-fsfreeze-*`, `guest-fstrim`, `guest-exec` and more, plus GuestKit's own evidence/health methods. Debian-family guests (it ships as a `.deb`). |
| `qemu` | Stock `qemu-guest-agent` through the distro package manager (any family with that package). |
| `none` | Nothing. |

Unset uses the server default `ZORVIA_GUEST_AGENT` (itself `none` unless set). The Create VM
page has a **Guest agent** selector. The agent is installed by cloud-init on first boot, so
it needs the guest to reach the package source; the VM is usable immediately and the agent
connects a few minutes later (measured: about 2.5 to 3 minutes on the lab).

Your own cloud-config is kept: the install is merged into `write_files` and `runcmd`
(your commands run first). User data that is not `#cloud-config` is refused (HTTP 400) rather than
silently ignored.

### How the Zyvor agent is fetched and trusted

The guest downloads a **pinned release** (`GuestKit v1.2.4`) over HTTPS and runs
`sha256sum -c` against a pinned checksum before `dpkg -i`; a changed or tampered download is
not installed. For air-gapped or mirrored environments set on the Zorvia deployment:

| variable | meaning |
|---|---|
| `ZORVIA_GUEST_AGENT_URL` | `https://` URL of the `.deb` (URL-safe characters only) |
| `ZORVIA_GUEST_AGENT_SHA256` | its SHA-256 (64 hex) |
| `ZORVIA_GUEST_AGENT` | default for VMs created without `guest_agent` |

### The channel permission (why a udev rule is written)

KubeVirt's agent channel `/dev/virtio-ports/org.qemu.guest_agent.0` is `root:root 0600`, while
the packaged service runs as the unprivileged `zyvor-agent` user, so the agent starts but never
connects (`failed to open virtio channel`). GuestKit's package fixes this with a udev rule
([guestkit#41](https://github.com/zyvorai/guestkit/pull/41)); until a release contains it, Zorvia's
cloud-init writes the identical rule and triggers udev. The rule is harmless once the package ships it.

## What it enables, and what is not done yet

- Connected agent: `wait-ready` reports `agent_connected: true`; recovery drills accept the agent
  as boot evidence (they already fall back to the serial console without one). A drill sees the agent
  only when it is **on the VM's disk**: a guest whose root is an ephemeral containerdisk boots a fresh
  image when restored and the drill namespace blocks the network the cloud-init install needs, so such a
  drill falls back to the serial console (observed: `guest_agent=false`). VMs with persistent root disks keep the agent.
- KubeVirt freezes and thaws the guest filesystem around online snapshots when an agent is present. The packaged
  unit (v1.2.4) runs the agent unprivileged with no capabilities, so its `fsfreeze` fails with "Operation not permitted"
  and **every online snapshot of a VM with PVC-backed disks fails at the freeze** (measured: `guest-fsfreeze-freeze`
  errors on a fresh guest; with the drop-in below it returns frozen/thawed). Zorvia's cloud-init therefore writes
  `/etc/systemd/system/guestkit-agent.service.d/10-zorvia-freeze.conf` granting only `CAP_SYS_ADMIN` as an ambient
  capability. **VMs created with `guest_agent: zyvor` before this change need the same drop-in** (or `guest_agent: qemu`)
  before they can be snapshotted online; the proper fix is in GuestKit: its unit says privileged operations run in a separate `guestkitd-exec` helper, but
  the QGA freeze/thaw handlers in v1.2.4 run `fsfreeze` directly from the unprivileged process. Once that is fixed the
  drop-in is redundant.
- Not yet: Windows (the agent supports it, installed offline by GuestKit; no Zorvia path yet) and RPM
  guests for the Zyvor agent (no `.rpm` asset is published).

## Calling the agent from Zorvia

Zorvia reaches the agent's channel with `virsh qemu-agent-command` inside the VM's `virt-launcher`
pod (the `pods/exec` permission it already holds), sending GuestKit's JSON-RPC methods in the agent's
`guestkit-rpc` QGA envelope. Replies are QGA-shaped, so the channel is not wedged the way a raw
non-QGA reply would wedge libvirt's agent client. Arguments are an argv vector, never a shell string.
Only a fixed **read-only** method set is reachable; guest exec, file and configuration methods are not.

| API | what it returns | role |
|---|---|---|
| `GET /api/vms/{name}/guest/agent` | agent version/protocol, health, snapshot readiness (hooks, quiesce support) | `vm.power` |
| `GET /api/vms/{name}/guest/inventory/{packages\|users\|certificates\|containers\|security}` | what the guest itself reports | `vm.power` |

Both answer `409 GUEST_AGENT_NOT_CONNECTED` for guests without a connected agent (and `502` if the agent errors).
The inventories expose accounts, certificates and installed software, so they are not available to read-only roles.

### Application hooks around snapshots and backups

Before a snapshot (`POST /api/vms/{name}/snapshots`) and before an off-cluster backup, Zorvia asks a connected
Zyvor agent for `guestkit.getSnapshotReadiness`, which runs the guest's pre-snapshot hooks (database flush
scripts under `/etc/zyvor/hooks`) and reports whether quiescing is possible. The result is returned as
`guest_quiesce` on the snapshot and stored in the backup operation's result:

- `consistency: "application"`: quiesce supported and every hook succeeded;
- `"filesystem"`: quiesce supported but a hook failed;
- `"crash"`: the agent cannot quiesce.

Zorvia does **not** call the agent's `snapshot.prepare`/`complete` pair. Those freeze the filesystem and
rely on a watchdog to thaw; KubeVirt already freezes and thaws through the same agent around the snapshot
and the VolumeSnapshot takes minutes to become ready on Ceph, so an extra freeze would only lengthen the
pause and could be thawed under it. Guests without the Zyvor agent behave as before (`guest_quiesce` is `null`).

### Drill evidence

When a recovery drill boots a restored guest with the Zyvor agent connected, the drill result carries
`guest_probe` (agent version and its own health verdict) in addition to "agent connected".
