# Upgrade notes

## v0.3.2 → v0.3.3+ (Production Foundation)

### Breaking / required ops

1. **Auth Secret is no longer in manifests.** Create it before upgrade:
   ```bash
   ./scripts/create-auth-secret.sh
   ```
2. **Lab defaults refused outside lab mode.** Production must omit `ZORVIA_LAB_MODE` and use unique JWT/admin passwords.
3. **OIDC is opt-in.** Set `ZORVIA_OIDC_ENABLED=1` plus issuer/client/secret/redirect to enable
   PKCE + token exchange + JWKS. Without the enable flag, `ZORVIA_OIDC_*` is ignored.
4. **Shared `ZORVIA_API_KEY` ignored** unless `ZORVIA_LAB_MODE=1`. Prefer `POST /api/v1/api-tokens`.
5. **Viewer role** can no longer call mutating APIs (server-side RBAC).
6. **Rook bootstrap** privileges removed from the core ServiceAccount. Apply `deploy/rook-bootstrap-rbac.yaml` or Helm `rbac.rookBootstrap=true` only if needed.
7. **TOTP disable** now requires password + TOTP code; sessions are revoked via `token_version`.

### Compatible

- Existing SQLite `auth.db` migrates in place (`token_version`, `api_tokens` tables).
- KubeVirt VM CRs unchanged.
- Helm chart (`charts/zorvia`) is preferred for new installs; `deploy/k8s.yaml` remains for lab remote-deploy.

### Recommended verification

```bash
curl -sk https://<host>:30152/api/v1/health
curl -sk https://<host>:30152/api/v1/features | jq .
curl -sk https://<host>:30152/api/v1/auth/providers   # [] unless OIDC enabled
# Viewer must get 403 on POST /api/vms
# Enterprise plans return 403 without ZORVIA_EXPERIMENTAL + ZORVIA_FEATURE_*
```

### New optional surfaces (0.3.3+)

| Surface | How to enable | Docs |
|---------|---------------|------|
| OIDC SSO | `ZORVIA_OIDC_ENABLED=1` + issuer/client/secret/redirect | [OIDC.md](OIDC.md) |
| Enterprise plan APIs | `ZORVIA_EXPERIMENTAL=1` + `ZORVIA_FEATURE_*` | [PHASE5_ENTERPRISE.md](PHASE5_ENTERPRISE.md) |
| Feature registry | always on | [FEATURE_MATURITY.md](FEATURE_MATURITY.md) |

### Matrix

| Component | From | To |
|-----------|------|-----|
| zorvia | 0.3.2 | 0.3.3+ |
| Kubernetes | 1.28+ | 1.28+ |
| KubeVirt | 1.2+ | 1.2+ |
