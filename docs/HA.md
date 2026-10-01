# Control-plane availability

Zorvia's state (users, audit trail, durable operations, schedules) is SQLite and JSON
files under `/data`, on one PersistentVolumeClaim. That decides the supported topology.

## Supported: one replica, fast failover (active/standby)

`replicaCount: 1` with a `ReadWriteOnce` volume (the default; also `values-production.yaml`).
If the node dies, Kubernetes reschedules the pod and the volume re-attaches elsewhere.
Expect an outage of roughly the time the CSI driver takes to detach/attach plus pod start
(minutes, not seconds, and longer if the dead node must first be fenced). While the API is
down, running VMs are unaffected.

What survives a restart or failover: sessions (token versions live in the DB), the audit
trail, durable operations (the reconciler resumes them; restarts do not consume retry
attempts), schedules and backup history.

The chart uses `strategy: Recreate` on RWO so the new pod is not blocked by a
Multi-Attach error. Upgrades therefore have a short API outage too.

## Not supported: two replicas on a ReadWriteOnce volume

Two writers on one SQLite file are unsafe, and an RWO volume cannot be mounted from two
nodes anyway. The chart fails to render when `replicaCount > 1` and `persistence.accessModes`
has no `ReadWriteMany`. Even on RWX, active/active is not a tested topology; leader election
(`Lease zorvia-schedulers`) only keeps background loops single-instance.

## Planned

A PostgreSQL backend for users, audit and operations would allow true active/active. It is
not part of the current release.

Measured so far: an API pod restart (`kubectl rollout restart`) takes about 40 to 50 s to a healthy API on the reference lab, with sessions and VMs unaffected ([SUPPORT_MATRIX.md](SUPPORT_MATRIX.md)). That is a planned restart, not node loss; no node-loss time has been recorded.

## Node-loss test (lab)

Cordon-less failure test: power off or `kubectl drain --force --delete-emptydir-data` the
node running `zorvia-api`, time until `GET /api/v1/health` answers again, then confirm
sessions still work and a running operation (`GET /api/operations`) resumes. Record the
measured time against your storage class; it is the number to quote as your RTO.
