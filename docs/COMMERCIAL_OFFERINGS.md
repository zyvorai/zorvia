# Commercial offerings

Zorvia is Apache-2.0 and every existing capability works without a subscription. Contracts describe **purchased services and support coverage**. They never gate VM features.

> **Expiry safety rule.** An expired, cancelled or missing contract changes commercial service eligibility only. It never stops VMs, disables VM APIs or blocks access to your data.

## Offerings

| Offering | Included scope | Billing model |
|---|---|---|
| Community | Software, documentation, community issues | Free |
| Supported | Technical assistance, upgrade guidance, agreed response times | Annual contract |
| Managed | Supported plus agreed monitoring, maintenance, upgrades, recovery testing | Monthly or annual contract |
| Deployment | Installation, integrations, configuration, acceptance testing | One-time project |
| Migration | Assessment, execution, guest validation, handover | Scoped project |
| Training | Administrator onboarding, operational workshops | Workshop fee |

The full per-offering detail (prerequisites, responsibilities, exclusions, required integrations) is served, without prices, by the public `GET /api/v1/commercial/catalog`. Prices are set per quote by authorized commercial administrators. Nothing is priced by default and nothing is charged automatically.

## Flow

1. **Quote request.** A signed-in user submits company, contact, cluster count, worker-node count, workload size, region, desired coverage and requirements. This creates a record. No notification is sent unless a notification integration is configured (none exists yet).
2. **Reviewable quote.** An administrator turns the request into a *draft contract*. The quote lists services and infrastructure covered, included capacity, pricing unit, currency, duration, response targets, support hours, both parties' responsibilities, exclusions and required integrations.
3. **Entitlement.** While the contract is a draft the administrator sets covered clusters, tier, coverage hours and timezone, authorized contacts, effective and expiration dates, node allowance and managed-operation permissions.
4. **Acceptance.** `draft → awaiting_acceptance`; a member of the customer organization accepts, giving `active`. Accepted terms cannot be edited; to revise, return the contract to `draft`.
5. **End of life.** `active → expired` (by date) or `cancelled`. Both are terminal; renewal is a new contract. Payment status (`not_invoiced`, `invoiced`, `paid`, `overdue`, `waived`) is tracked separately from contract status.

Activation requires effective and expiry dates, plus for Supported/Managed at least one covered cluster, a timezone and coverage hours.

## Organizations and isolation

Customers are organizations with named members (usernames). Non-administrators see only quote requests they filed or that belong to their organizations, and only their organizations' contracts, history and coverage. Anything else answers `404`, indistinguishable from a missing record. Administrators (`users.admin`) see everything.

## API

| Endpoint | Access |
|---|---|
| `GET /api/v1/commercial/catalog` | Public |
| `GET/POST /api/v1/commercial/quote-requests` | Signed in |
| `GET /api/v1/commercial/contracts`, `/{id}`, `/{id}/history` | Signed in, own organizations |
| `POST /api/v1/commercial/contracts/{id}/accept` | Member of the contract's organization |
| `GET /api/v1/commercial/coverage[?cluster=]` | Signed in, own organizations |
| `/api/v1/commercial/admin/*` | `users.admin` only: organizations and members, quote generation, entitlement edits, transitions, payment status, offline import |

An unconfigured integration shows as unavailable in the catalog response (`integrations.*.available = false`).

### Offline import

`POST /api/v1/commercial/admin/contracts/import` loads a contract bundle for disconnected deployments. It is administrator-only, validated like any contract, recorded in history with `source = offline_import`. **Bundle authenticity is not cryptographically verified in this phase.**

## Storage, backup and rollback

- Commercial data lives in its own SQLite file: `ZORVIA_COMMERCIAL_DB` (default `<data dir>/zorvia/commercial.db`), separate from VM state and the auth DB.
- **Single instance only.** SQLite in one file is safe for exactly one commercial API replica. A shared transactional store is required before running several replicas; that store is not implemented yet.
- **Migrations** are versioned and append-only (`schema_migrations`), applied automatically at first use, each in a transaction.
- **Backup:** before upgrading, stop the server (or use `sqlite3 commercial.db ".backup 'commercial.pre-upgrade.db'"`) and keep the copy.
- **Rollback:** there are no down-migrations. Stop the server, restore the pre-upgrade copy, start the older version. A build refuses to open a database whose schema is *newer* than it understands rather than run against it.
- Every contract change (quote, entitlement edits with before/after, transitions, acceptance, payment status, import, expiry) is appended to the contract's history with actor and time.

## Feature maturity

| Area | Status |
|---|---|
| Catalog, quote requests, contracts, coverage, offline import | **Beta** (records only) |
| Support cases and diagnostic exports | Not implemented |
| Service engagements, managed-operation records | Not implemented |
| Capacity reconciliation, invoice records | Not implemented |
| Payment provider integration | Not implemented; waits on provider and terms |
| Notifications | Not implemented; requests are recorded only |

Existing experimental migration APIs remain experimental (see [FEATURE_MATURITY.md](FEATURE_MATURITY.md)).
