# Billing units

> **Status: design only.** No capacity observations, invoices or payment processing exist in this release, and nothing is charged. The contract fields that will feed them (`node_allowance`, and the `node_treatment` statement on each quote) are already recorded ([COMMERCIAL_OFFERINGS.md](COMMERCIAL_OFFERINGS.md)).

## Proposed unit

One **registered Kubernetes worker node** within an agreed covered cluster.

Each contract states how these are treated: control-plane nodes, infrastructure-only nodes, temporary nodes, and disconnected clusters. Quotes carry this in `node_treatment`.

## Planned observations

Stable cluster identity, covered worker-node count, timestamp and source, and any discovery error or stale-data flag. No guest contents or workload names are collected for billing.

**A missing or stale observation is never a zero-node claim.** It is recorded as unknown and reconciled, not billed as zero or as any invented number.

## Planned invoices and payments

Invoice records with line items, currency, billing period, status, external reference and downloadable documents. Tax and issuance stay with the billing integration. Payments will go through provider adapters that verify webhook signatures, reject replays and process events idempotently. No card details are stored, and recurring charges require an accepted agreement and payment authorization.
