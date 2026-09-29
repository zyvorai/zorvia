# Adopting an existing KubeVirt cluster

**Status: untested against OpenShift Virtualization.** Zorvia is verified on plain Kubernetes with KubeVirt (lab cluster). The OpenShift-specific points below are things to check, not results.

Zorvia reads and writes standard KubeVirt objects, so an existing cluster's VMs appear without conversion. Go in stages.

## 1. Read-only first

Create a kubeconfig or ServiceAccount that can only `get`/`list`/`watch` these resources:

- `virtualmachines`, `virtualmachineinstances` (`kubevirt.io`)
- `datavolumes` (`cdi.kubevirt.io`)
- `persistentvolumeclaims`, `nodes`, `namespaces`

Install with the lab values, then sign in and confirm the VM list matches `oc get vm -A`:

```bash
helm upgrade --install zorvia charts/zorvia -n zorvia-system --create-namespace \
  -f charts/zorvia/values-lab.yaml
```

## 2. Check on OpenShift

- **Security context constraints:** the pod may be rejected under the default `restricted` SCC. Check the pod events and the chart's `securityContext`.
- **Exposure:** the guide uses a NodePort. On OpenShift you would normally use a Route. Zorvia serves HTTPS itself, so use passthrough termination.
- **Storage class and CDI:** confirm the storage classes your VMs use exist and that CDI is installed.

## 3. Then day-2

Once the read-only view is right, grant write roles per the RBAC in [WEB_CONSOLE.md](WEB_CONSOLE.md) and try snapshots and live migration on a non-critical VM first.

## Limits

- Live migration needs shared storage (same as [FEATURE_MATURITY.md](FEATURE_MATURITY.md) notes).
- Moving VMs to a different cluster is not a shipped Zorvia feature. For that, [talk to sales](https://zyvor.dev/contact?intent=sales&utm_source=github&utm_medium=zorvia&utm_campaign=docs_adopt) about commercial support and services.

Back to [Leaving OpenShift](leave-openshift.md).
