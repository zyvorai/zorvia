# Support scope

> **Status: contract terms are implemented; the support case portal is not.** Coverage hours, timezone, authorized contacts and response targets are recorded on contracts today ([COMMERCIAL_OFFERINGS.md](COMMERCIAL_OFFERINGS.md)). Support cases, timers and diagnostic exports are planned for the next delivery phase.

## What a Supported contract covers

- Technical assistance for the **covered clusters** named in the entitlement.
- Upgrade guidance.
- Response within the targets written into the contract.

Community users get documentation and GitHub issues, best effort, with no response commitment.

## Not covered

Remote access to your clusters, third-party software Zyvor does not ship, and anything the contract lists under exclusions.

## Response targets vs resolution estimates

A **response target** is how soon a person engages after a case is opened. A **resolution estimate** is a non-binding expectation of when the problem is fixed. They are defined separately in the contract; only response targets carry commitments.

## Planned timer rules (not yet implemented)

- Timers run only inside the contract's coverage hours, in its IANA timezone, excluding agreed holidays.
- The clock pauses while a case is `waiting_on_customer`.
- The contract snapshot at case creation decides which targets apply.

## Planned case lifecycle

`open → triaged → investigating → waiting_on_customer → resolved → closed`, with reopening, escalation history and organization-isolated access.
