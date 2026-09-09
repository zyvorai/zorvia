# Change Planner

`zorvia plan` turns Drift Guard findings into an operational execution plan.

## Examples

```bash
# Offline desired-vs-actual planning
zorvia plan desired.yaml --actual live-capture.yaml

# Plan against the live cluster
zorvia --namespace production plan desired.yaml --vm payments-01

# CI gates
zorvia plan desired.yaml --actual current.yaml --fail-on-downtime
zorvia plan desired.yaml --actual current.yaml --fail-on-recreate

# Machine-readable output
zorvia plan desired.yaml --actual current.yaml -o json
```

The planner is intentionally conservative. CPU/memory and network changes default to a controlled restart unless the operator confirms hotplug support. Storage-source, firmware, machine-type and TPM changes require recreation or explicit review.
