# CDI Golden Images

`zorvia image-bundle` generates a versioned CDI `DataVolume` plus a stable `DataSource` alias. This lets VMs consume a stable image name while image promotion remains rollback-friendly.

```bash
zorvia --namespace vm-images image-bundle ubuntu-golden \
  --version 24.04-r2 \
  --source https://images.example/ubuntu.qcow2 \
  --source-type http \
  --size 40Gi \
  --storage-class fast \
  --checksum sha256:abc123 \
  --output ubuntu-golden.yaml
```

Promotion flow: import the versioned DataVolume, wait for CDI success, validate with a disposable VM, then apply the stable DataSource alias. Retain the prior versioned PVC for rollback.
