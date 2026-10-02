# GPUs, SR-IOV and other passthrough devices

Zorvia can create VMs that hold a physical or mediated device, and checks *before* creating the VM that
the cluster can actually give it that device.

> **Status: Experimental, not run on real hardware.** The VM spec generation, the inventory and the
> preflight are unit-tested, and the API behaviour was checked on a lab that has **no GPU and no SR-IOV
> NIC**. A trial on your own GPU/SR-IOV hardware is still needed before relying on it.

## Creating a VM with a device

`POST /api/vms` takes `devices`:

```json
{ "name": "gpu-vm", "image": "...", "cpus": 8, "memory": 16384,
  "devices": [
    { "name": "gpu0", "device_name": "nvidia.com/GA102GL_A10", "kind": "gpu" },
    { "name": "nic0", "device_name": "intel.com/x710", "kind": "host-device" } ] }
```

`device_name` is the extended resource your **device plugin** advertises. `kind` `gpu` (default) is emitted
as KubeVirt `spec.domain.devices.gpus[]`, `host-device` as `hostDevices[]` (any other PCI, mediated or USB
device). At most 16 per VM; names are DNS labels and unique; resource names look like `vendor.com/name`.
The Create VM page has a **Passthrough devices** section (suggestions come from the inventory when you are a
cluster admin).

**SR-IOV networks** use the existing `interfaces` field: `{"name":"vf0","network":"vf-net","model":"virtio",
"network_type":{"sriov":{"name":"vf-net"}}}` (the **Additional NICs** section's SR-IOV option), a Multus
`NetworkAttachmentDefinition` in the VM's namespace. KubeVirt gets `interfaces[].sriov` on a `multus` network.

## Preflight (and why creation can answer 422)

A VM that asks for hardware nobody offers would be created and then sit `Pending`. Zorvia checks first and
answers **422 `DEVICE_PREFLIGHT_FAILED`** with the reasons and an `issues` list. It checks:

| check | error when |
|---|---|
| KubeVirt permission | the resource is not in the KubeVirt CR's `spec.configuration.permittedHostDevices` (KubeVirt would refuse it) |
| availability | no node advertises it (device plugin missing, hardware absent), or every unit is already in use |
| SR-IOV: Multus | the `NetworkAttachmentDefinition` CRD is not installed |
| SR-IOV: network | the attachment does not exist in the VM's namespace |
| SR-IOV: VF pool | the attachment's `k8s.v1.cni.cncf.io/resourceName` pool has no free virtual functions on any node |

Warnings (never block): the attachment has no `resourceName` annotation (the pool cannot be checked), and
**a VM with a passthrough device or SR-IOV interface is pinned to its node**: it cannot be live-migrated and a
node drain will stop it ([MAINTENANCE.md](MAINTENANCE.md) flags such VMs). If the KubeVirt CR cannot be read the
permission check is skipped rather than guessed.

`POST /api/v1/devices/preflight` runs the same check without creating anything
(`{"devices":[...], "sriov_networks":["vf-net"], "namespace":"..."}` -> `{"ok":false,"issues":[...]}`).
`GET /api/v1/devices` (cluster.admin) is the inventory: per node the device-plugin resources and free counts,
KubeVirt's builtin pseudo-devices, the permitted devices and feature gates, and whether Multus is installed.

## What you have to set up (Zorvia does not)

1. A **device plugin** for the hardware (NVIDIA GPU operator / KubeVirt GPU device plugin, the SR-IOV network
   device plugin, ...) so nodes advertise the resource.
2. The device in the KubeVirt CR's `spec.configuration.permittedHostDevices` (`pciHostDevices`,
   `mediatedDevices` or `usb`).
3. For SR-IOV: Multus and a `NetworkAttachmentDefinition` whose annotation `k8s.v1.cni.cncf.io/resourceName`
   names the VF pool. On older KubeVirt versions, the `GPU` / `HostDevices` feature gates.
4. Host prerequisites for passthrough (IOMMU enabled, VFIO binding) are the node's job.

Zorvia's service account needs read access to the `kubevirts` resource (added to the Helm chart and the
manifests: `get`, `list` only) and `get` on `network-attachment-definitions`.

## Not covered

NUMA/CPU-pinning topology constraints, GPU sharing/MIG policy, hotplugging a device into a running VM, scheduling
a VM onto the node that holds a specific unit, and any validation on real hardware.
