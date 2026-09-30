# VMware → KubeVirt import (h2kvm)

Zorvia drives [h2kvm](https://github.com/zyvorai/h2kvm) to move VMs from
vCenter into KubeVirt. For each VM it runs
`h2kvmctl --cmd vsphere … --deploy-k8s` as a Kubernetes Job: export from vCenter,
GuestKit offline repair (drivers, fstab, boot), convert to qcow2, upload through
CDI, create the `VirtualMachine`. Zorvia adds the parts around it: parameters,
credentials, scratch space, waves with a concurrency cap, progress, boot
verification, and rollback.

h2kvm is separately licensed and **not shipped with Zorvia**: you supply the
image (`ZORVIA_H2KVM_IMAGE`). Zorvia only runs it as a container; it never links
it.

## Set up

1. Set `ZORVIA_H2KVM_IMAGE` on the Zorvia deployment to your h2kvm CLI image
   (optional: `ZORVIA_H2KVM_INIT_IMAGE`, default `fedora:43`, used to load the
   `nbd` kernel module).
2. Prepare each target namespace (see `deploy/h2kvm-import-rbac.yaml`): label it
   `pod-security.kubernetes.io/enforce=privileged`, create the vCenter
   credentials Secret (`username` / `password` keys) and the `zorvia-h2kvm`
   ServiceAccount + Role. Zorvia has no access to Secrets or ServiceAccounts, so
   preflight can only remind you of these.
3. KubeVirt and CDI must be installed, and the cluster needs somewhere to hold
   scratch space: the Job gets a per-import ephemeral volume
   (`scratch_size`, default 200Gi). It must hold the largest disk about twice
   over (the export and the converted image).

## Use

All routes are cluster.admin (they start **privileged** Jobs and read vCenter
credentials) and are audited.

```
POST /api/vm-imports/preflight   # dry run: blockers and warnings
POST /api/vm-imports             # queue a wave (one operation per VM)
GET  /api/vm-imports             # list imports
GET  /api/vm-imports/waves/{id}  # counts by state + per-VM progress
GET  /api/operations/{op}        # one import; POST .../cancel to stop + roll back
```

```json
{
  "source": {"vcenter": "vc.example.com", "datacenter": "DC1", "insecure": false,
             "secret_name": "vcenter-creds"},
  "namespace": "vms",
  "scratch_size": "300Gi",
  "vms": [
    {"source_vm": "web-prod-01", "storage_class": "longhorn", "pvc_size": "60Gi",
     "cpu": 4, "memory": "8Gi", "start": true},
    {"source_vm": "db-prod-01", "target_vm_name": "db-01"}
  ]
}
```

- `POST /api/vm-imports` runs preflight first and answers **409** with the
  blockers if anything would stop it (missing KubeVirt/CDI, namespace enforcing
  a Pod Security level that rejects privileged pods, unknown StorageClass, target
  name already taken, no image configured).
- A wave runs `ZORVIA_IMPORT_CONCURRENCY` imports at a time (default 2); the rest
  stay `queued`.
- Without `start`, the imported VM is left stopped so you can cut over on your
  schedule. With `start`, Zorvia waits (`ZORVIA_IMPORT_BOOT_TIMEOUT_SECS`,
  default 600) for the VM to reach Running. A missing guest agent is reported,
  not treated as a failure (VMware guests rarely have it).

## What "succeeded" means, and rollback

An import succeeds only when the Job finished **and** the `VirtualMachine`
exists in the cluster (Zorvia checks the API, not just h2kvm's log). If the Job
fails, times out, or you cancel, Zorvia deletes the Job, the VM, and the root
PVC/DataVolume named after it. That is safe because the target name is verified
free before the Job starts. If the VM was created but did not boot, it is left
in place for inspection and the operation is marked failed.

Progress is coarse: h2kvm logs stage boundaries, not bytes, so the percentage
advances per completed stage.

## Limits

- Not yet exercised against a real vCenter (it needs one plus the h2kvm image).
- Network mapping (port groups → NetworkAttachmentDefinitions) is not exposed
  yet; VMs come up with h2kvm's default networking.
- Credentials reach h2kvm through `secretKeyRef` environment variables and are
  never stored in the operation record.
