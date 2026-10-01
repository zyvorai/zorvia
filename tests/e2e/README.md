# Real-guest E2E

`guest.sh` boots a Linux guest through the Zorvia API and exercises it: create + boot + guest
agent, snapshot, live migration under CPU/disk load (needs two schedulable nodes and `KUBECTL`),
and an API restart that must leave the session and the VM untouched. Output is a markdown table
meant for `docs/SUPPORT_MATRIX.md`; a scenario that cannot run reports SKIP with the reason, never
a silent pass.

```bash
ZORVIA_LAB_HOST=80.79.5.173 ZORVIA_E2E_PASSWORD=... \
KUBECTL="ssh root@80.79.5.173 KUBECONFIG=/etc/rancher/k3s/k3s.yaml kubectl" \
tests/e2e/guest.sh
```

Not covered yet: Windows guests, node failure, upgrades, off-cluster backup/restore/drill (those
are exercised separately, see `docs/OFFCLUSTER_BACKUP.md`). CI runs it on demand through the
`guest-lab` job of `e2e-kubevirt.yml` on a self-hosted `zorvia-lab` runner.
