# Guest Insight

`zorvia guest-insight <vm>` summarizes KubeVirt VMI guest integration. It reports phase, node, QEMU Guest Agent detection, guest OS/kernel metadata and observed interfaces without logging into the VM.

```bash
zorvia --namespace production guest-insight payments-01
zorvia --namespace production guest-insight payments-01 -o json
zorvia --namespace production guest-insight payments-01 --strict
```

`--strict` returns a non-zero result when the QEMU Guest Agent is not detected.
