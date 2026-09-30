# Managed operations

> **Status: not implemented.** Today a Managed contract records the purchased tier and `managed_permissions` ([COMMERCIAL_OFFERINGS.md](COMMERCIAL_OFFERINGS.md)); nothing operates a cluster. This page is the design contract for the delivery phase.

## Principles

1. **A contract grants no cluster access.** Purchasing Managed does not let anyone reach your cluster.
2. **Explicit enrollment.** You enroll each cluster yourself.
3. **Remote operation is a separate switch.** It is off by default and uses scoped, revocable credentials that you issue and can withdraw at any time.
4. **Automated remediation needs all of:** a documented policy, permissions, an audit trail and your authorization.
5. **Expiry never affects your VMs.** A lapsed contract ends the managed service; it does not stop workloads.

## Planned records

Covered components and maintenance windows, available monitoring signals, the assigned operational owner, approved maintenance tasks, backup and restore-test evidence, open incidents and service reports.
