# Off-cluster backup, restore and recovery drills

A backup normally ends as a cluster-local `VirtualMachineSnapshot`. With an S3
target configured, the backup operation continues: each volume snapshot is
restored to a temporary PVC, streamed to S3 by the `backup-agent` Job
(checksummed, optionally AES-256-GCM encrypted, Object Lock aware), read back
and re-verified, and only then is the manifest published.

## Configure (environment on the Zorvia deployment)

| Variable | Purpose |
|---|---|
| `ZORVIA_BACKUP_S3_ENDPOINT`, `_BUCKET`, `_SECRET` | Required to enable off-cluster backups |
| `ZORVIA_BACKUP_S3_REGION`, `_PREFIX` | Optional (`us-east-1`, empty) |
| `ZORVIA_BACKUP_S3_ACCESS_KEY_KEY`, `_SECRET_KEY_KEY` | Key names inside the Secret (`access-key`, `secret-key`) |
| `ZORVIA_BACKUP_ENCRYPTION_KEY_KEY`, `_ID` | Secret key holding a 64-hex-char AES-256 key; label recorded in the manifest |
| `ZORVIA_BACKUP_LOCK_MODE` (`GOVERNANCE`/`COMPLIANCE`), `_LOCK_DAYS` | Object Lock retention applied to every object |
| `ZORVIA_BACKUP_PART_MB`, `_JOB_DEADLINE_SECS` | Multipart size (default 64) and Job deadline (default 6h) |
| `ZORVIA_BACKUP_AGENT_IMAGE` | Agent image (`deploy/backup-agent.Dockerfile`) |

### Using an Atlas bucket as the target

If you run [Atlas](https://github.com/zyvorai/atlas), point Zorvia at one of its
buckets instead of typing the endpoint, bucket and Secret yourself:

```
ATLAS_URL=http://atlas-gateway.zyvor-system:5110      # already needed for the Atlas integration
ZORVIA_BACKUP_ATLAS_BUCKET=bkt_9624f5a6596f            # id from GET /api/atlas/v1/buckets
```

Zorvia reads the bucket record from Atlas (it must be `bound`) and takes the
S3 `endpoint`, `bucket_name`, `region` and credential `secret_ref` from it. Atlas
buckets are Rook ObjectBucketClaims, so the Secret uses the `AWS_ACCESS_KEY_ID` /
`AWS_SECRET_ACCESS_KEY` keys; Zorvia defaults to those names in this mode.
Prefix, encryption, Object Lock and part size still come from the
`ZORVIA_BACKUP_*` variables above. Atlas's object storage is Ceph RGW (or any
S3 endpoint you register with it), so this needs a working RGW behind Atlas.

The Secret lives in the bucket's namespace (the OBC namespace, e.g.
`rook-ceph`), but the Job runs in the VM's namespace, so copy it there (Zorvia
does not read Secrets).

The credential **Secret must exist in the namespace of every VM you back up**
(and in the drill namespace). Zorvia deliberately has no cluster-wide Secret
access; a missing Secret fails the operation immediately with a clear message.
Keep the encryption key somewhere other than the bucket: without it an
encrypted backup cannot be restored.

Verified end to end (encrypted backup, read-back, restore, recovery drill) on KubeVirt v1.9.0 / CDI v1.66.0 with Rook-Ceph RBD and an Atlas-provisioned RGW bucket; see [SUPPORT_MATRIX.md](SUPPORT_MATRIX.md) for versions and timings. Block-mode volumes are not supported (create DataVolumes with `volumeMode: Filesystem`; on RBD CDI otherwise defaults to Block).

## Back up now

`POST /api/backups/offcluster` with `{"vm_name": "...", "namespace"?, "retention_days"?, "description"?}`
queues the same durable operation the scheduler uses (snapshot, upload, read-back verify) and returns
`{"operation_id"}` to poll on `GET /api/operations/{id}`. It answers 409 if no target is configured.
Scheduled backups use it automatically once a target is set.

### Block-mode volumes

Source volumes with `volumeMode: Block` (the default on some RBD classes) are supported for backup. The temp PVC stays Block and is attached to the agent Job as a raw device; the agent measures it by seeking to the end and streams it. Because the device node is `root:disk 0660`, Jobs with a Block source run the agent as root (still `privileged: false`, no privilege escalation, every capability dropped); filesystem-only Jobs keep the unprivileged CDI UID. A restore always writes the content back as a `disk.img` file on a Filesystem PVC. Verified on Ceph RBD: the SHA-256 of the restored file equals that of the source device. Restoring *into* Block volumes is not supported.

## Restore

`GET /api/backups/offcluster` lists restorable backups.
`POST /api/backups/offcluster/{operation_id}/restore` with
`{"new_vm_name": "...", "namespace"?, "storage_class"?, "start"?}` restores the
disks into new PVCs (each part verified before it is written; existing files
are never overwritten) and creates a new VM from the saved spec. Firmware
uuid/serial and MAC addresses are dropped so it cannot collide with the
original. The VM is created halted unless `start` is true. Poll
`GET /api/operations/{id}`.

## Recovery drills

`POST /api/backups/offcluster/{operation_id}/drill`, or set
`ZORVIA_DRILL_INTERVAL_HOURS` to drill the latest backup of every VM on a
schedule. A drill restores into `ZORVIA_DRILL_NAMESPACE` (default
`zorvia-drill`, with a NetworkPolicy that denies all traffic to and from the restored guest's virt-launcher pod; the restore Job in the same namespace is unaffected), replaces the VM's
networks with a bare pod network, boots it, waits for the guest agent
(`ZORVIA_DRILL_BOOT_TIMEOUT_SECS`, default 600), then deletes the VM and its
volumes. The operation result records restore and boot times. A drill proves
the backup restores and the guest boots with a working agent; it does not run
application-level checks.

## Retention

Cluster-local backup snapshots are deleted hourly once past their
`retention_days`; the newest backup of each VM is always kept. Off-cluster
objects are protected by Object Lock at write time and expire through a bucket
lifecycle rule -- Zorvia holds no delete rights on the store.

## Limits

Filesystem-mode volumes only (block-mode volumes are rejected). Restores read
`<pvc>/disk.img` -- verify the layout and that the agent's UID (107 by
default, `ZORVIA_BACKUP_AGENT_UID`) can read it on your storage class before
relying on this in production.
