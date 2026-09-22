# Zorvia Helm chart

Production-oriented packaging for the Zorvia API/console.

## Lab install

```bash
./scripts/create-auth-secret.sh
helm upgrade --install zorvia ./charts/zorvia \
  -n zorvia-system --create-namespace \
  -f charts/zorvia/values-lab.yaml \
  --set auth.existingSecret=zorvia-auth \
  --set api.exposeHost=<node-ip>
```

## Production install

```bash
./scripts/create-auth-secret.sh   # unique credentials; do NOT set ZORVIA_LAB_MODE
helm upgrade --install zorvia ./charts/zorvia \
  -n zorvia-system --create-namespace \
  -f charts/zorvia/values-production.yaml \
  --set auth.existingSecret=zorvia-auth \
  --set image.repository=ghcr.io/zyvorai/zorvia \
  --set image.tag=0.3.3 \
  --set ingress.hosts[0].host=zorvia.example.com
```

## Features

| Value | Lab | Production |
|-------|-----|------------|
| replicas | 1 | 2 |
| TLS | in-pod self-signed | Ingress + cert-manager |
| Service | NodePort 30152 | ClusterIP |
| PDB / NetworkPolicy / anti-affinity | off | on |
| Leader election (Lease) | on | on |
| Persistent audit DB | `/data/audit.db` | same |
| Rook bootstrap SA | opt-in (`rbac.rookBootstrap`) | opt-in |

Raw manifests in `deploy/k8s.yaml` remain for the existing remote-deploy path.
