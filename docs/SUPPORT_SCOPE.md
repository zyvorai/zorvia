# Support scope

> Prices are in [PRICING.md](PRICING.md). Response targets and support hours are **not yet published**: they are published only after being confirmed against staffing capacity. Zyvor does not promise resolution deadlines, and does not offer 24×7 coverage by default.
>
> **Product status:** contract terms (coverage hours, timezone, authorized contacts, covered clusters) are recorded today ([COMMERCIAL_OFFERINGS.md](COMMERCIAL_OFFERINGS.md)). The support case portal is separate work and is documented at the end of this page.

Support is a paid service contract around the Apache-2.0 software. It is not a software licence and nothing in the product enforces it.

## Tiers

| | Supported | Supported Plus | Managed |
|---|---|---|---|
| Zorvia troubleshooting and upgrade guidance | Business hours | Business hours, priority handling | Per contract |
| Quarterly health review | No | Yes | Per contract |
| Planned Zorvia upgrade assistance | Guidance | Assistance | Per contract |
| Monitoring, maintenance, operational tasks | No | No | Per contract |

## What is covered

| Component | Supported / Supported Plus | Managed |
|---|---|---|
| Zorvia API, CLI, TUI and web console | Covered | Covered |
| Zorvia configuration and documented integrations | Covered within the agreed compatibility matrix | Covered |
| Kubernetes and KubeVirt operations | Diagnostic guidance; operational ownership priced separately | Covered only when explicitly contracted |
| Storage and networking | Zorvia integration troubleshooting; backend support priced separately | Defined per contract |
| Guest operating systems and applications | Separately scoped | Separately scoped |
| Backups and recovery drills | Separately scoped | Included only where contracted and validated |
| Hardware and cloud infrastructure | Customer / provider responsibility | Responsibility specified in the contract |

Experimental features ([FEATURE_MATURITY.md](FEATURE_MATURITY.md)) are outside support commitments unless the contract says otherwise in writing.

## Support hours, timezone and channels

To be published once confirmed against staffing capacity:

| Item | Value |
|---|---|
| Support hours | *Not yet published* |
| Timezone | *Not yet published* |
| Contact channels | *Not yet published* (interim: sales@zyvor.dev) |
| Response targets by severity | *Not yet published* |

Per contract, hours and timezone are recorded in the entitlement and are what timers will use.

## Severity definitions (proposed)

| Severity | Meaning |
|---|---|
| 1 – Critical | Production Zorvia function unavailable or unusable, with no workaround |
| 2 – High | Major Zorvia function badly degraded, or Sev 1 with a workaround |
| 3 – Medium | Partial loss of a non-critical function, with a workaround |
| 4 – Low | Question, documentation issue or feature request |

Severity describes impact on Zorvia functionality. Issues in Kubernetes, storage, hardware or guests that Zorvia merely reveals are handled as diagnostic guidance unless separately contracted.

## Response targets vs resolution estimates

A **response target** is how soon a person engages after a case is opened. A **resolution estimate** is a non-binding expectation. They are defined separately; only response targets can carry a commitment, and only once published.

## Support case portal (not yet implemented)

Planned: cases with `open → triaged → investigating → waiting_on_customer → resolved → closed`, reopening, escalation history, contract-coverage snapshot, and organization-isolated access. Timers will run only inside the contract's coverage hours and timezone, skip agreed holidays, and pause while a case waits on the customer.
