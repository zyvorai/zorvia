# PostgreSQL backend

Status: **Beta, phases 1-3 (auth store, operations queue, audit trail).**

## What moves

| Store | Backend with `ZORVIA_DATABASE_URL` set |
|---|---|
| Users, API tokens, session revocations, TOTP state, login throttle | **PostgreSQL** |
| Durable operations (`ops.db`): queue, state, progress, results | **PostgreSQL** |
| Audit trail (`audit.db`) | **PostgreSQL** (every replica writes to it; reads refresh at most once a second) |
| Schedules, warm pools, alerts and other JSON state | Files on the data volume |

Because the JSON state files (schedules, warm pools, alerts, ...) are still per replica, **running two active replicas is not yet supported.** What you get now:
sign-in state and the operation queue survive the loss of the data volume and are the same for every replica. Token revocation
and TOTP replay protection hold across replicas, an idempotency key creates one operation even when two replicas race on it,
and only one replica can claim an operation (all covered by the race tests below).

### Operations on a shared store

Only the lease leader runs operations. At start-up the leader re-queues work left `running` by a dead process, but on PostgreSQL it
leaves alone any operation whose heartbeat is newer than 90 seconds, so a replica that starts while another is healthy does not
steal its work; a genuinely orphaned operation is picked up by the normal 120 s orphan check.

## Configure

```yaml
database:
  existingSecret: zorvia-db   # Secret with key "url": postgres://user:pass@host:5432/zorvia
  import: true                # first start only
```

or `ZORVIA_DATABASE_URL=postgres://...` (`sslmode=require` enables TLS). The schema is created on start, guarded by a
Postgres advisory lock so replicas starting together do not race.

## Migrating from SQLite

Start once with `ZORVIA_DATABASE_IMPORT=1`: users, tokens and revocations are copied from `auth.db`, and the audit history from `audit.db`, into **empty** tables.
It is idempotent and does nothing when the database already has users. Remove the flag afterwards. The SQLite file is left
untouched, so rollback is unsetting the URL.

## Tests

`cargo test` runs the same conformance and race suite on SQLite always, and on Postgres when `ZORVIA_TEST_POSTGRES_URL` is set
(CI-less today: run it against a throwaway container). Live-verified on the lab cluster (single node, Postgres 17 in-cluster): import of the existing admin, sign-in, restart persistence, and two replicas where a logout on one replica made the token fail on the other. Not verified: Postgres failover, TLS-enabled connections to a managed service.

## Limits

- No automatic failover of PostgreSQL itself; use your operator (CloudNativePG, Patroni, managed service).
- One statement is retried once on a dropped connection; there is no pool, each replica holds one connection.

## Audit trail notes

The API keeps the newest 10 000 entries in memory for filtering and refreshes them from PostgreSQL when the audit pages or the security
dashboard are read. The optional JSONL sidecar (`ZORVIA_AUDIT_JSONL`) is still written per replica to its own volume. A failed audit
write is logged as a warning and does not fail the request, as with SQLite.
