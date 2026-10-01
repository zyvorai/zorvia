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
- KubeVirt freezes and thaws the guest filesystem around online snapshots when an agent is present.
- Not yet: Windows (the agent supports it, installed offline by GuestKit; no Zorvia path yet), RPM
  guests for the Zyvor agent (no `.rpm` asset is published), calling GuestKit's own JSON-RPC methods
  from Zorvia (libvirt's `virsh qemu-agent-command` mis-handles non-QGA replies and wedges the
  channel, so they need a dedicated client), and application hooks around backups.
