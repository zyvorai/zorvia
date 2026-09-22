# OIDC / enterprise SSO (Beta)

Zorvia supports OpenID Connect as an **opt-in** login path. It is off unless
you set `ZORVIA_OIDC_ENABLED=1`. Without that flag, `ZORVIA_OIDC_*` variables
are ignored and `GET /api/v1/auth/providers` returns `[]`.

## Security model

The callback **never** mints a session from an authorization `code` alone.
The flow is:

1. `GET /api/v1/auth/oidc/:id` — OpenID discovery, create PKCE verifier + nonce + state
2. Browser redirects to the IdP authorize URL (`code_challenge` S256)
3. `GET /api/v1/auth/oidc/callback` — one-time state lookup, **token exchange**
   with `code_verifier`, fetch JWKS, verify `id_token` (`iss`, `aud`, `nonce`, `exp`)
4. JIT provision a local SQLite user `oidc:<sub>` (unusable password; IdP-only login)
5. Mint a normal Zorvia JWT via `JwtConfig::generate` (honors `token_version` / disable)

Admin RBAC and session revocation apply to OIDC users the same as password users.

## Configuration

| Variable | Required | Default |
|----------|----------|---------|
| `ZORVIA_OIDC_ENABLED` | yes (`1` / `true`) | off |
| `ZORVIA_OIDC_ISSUER` | yes | — |
| `ZORVIA_OIDC_CLIENT_ID` | yes | — |
| `ZORVIA_OIDC_CLIENT_SECRET` | yes | — |
| `ZORVIA_OIDC_REDIRECT_URI` | recommended | `https://127.0.0.1:30152/api/v1/auth/oidc/callback` |
| `ZORVIA_OIDC_NAME` | no | `OIDC` |
| `ZORVIA_OIDC_SCOPES` | no | `openid profile email` |

Redirect URI must match the IdP app registration exactly (including HTTPS and path).

### Helm / Kubernetes

Set the vars on the API Deployment (or Secret → env). Example:

```yaml
env:
  - name: ZORVIA_OIDC_ENABLED
    value: "1"
  - name: ZORVIA_OIDC_ISSUER
    value: "https://idp.example.com"
  - name: ZORVIA_OIDC_CLIENT_ID
    valueFrom: { secretKeyRef: { name: zorvia-oidc, key: client-id } }
  - name: ZORVIA_OIDC_CLIENT_SECRET
    valueFrom: { secretKeyRef: { name: zorvia-oidc, key: client-secret } }
  - name: ZORVIA_OIDC_REDIRECT_URI
    value: "https://zorvia.example.com/api/v1/auth/oidc/callback"
```

## Operator checks

```bash
# Providers empty when disabled
curl -sk https://HOST:30152/api/v1/auth/providers
# []

# Live maturity registry
curl -sk https://HOST:30152/api/v1/features | jq '.features[] | select(.id=="oidc")'
```

After enable, the sign-in page shows an OIDC button from `/api/v1/auth/providers`.
Successful callback redirects to `/sign-in?oidc_token=…` (SPA stores the JWT).

## Limitations

- Single provider (`id=default`) per process today
- No SAML
- JWT still appears in the redirect query string (prefer short-lived exchange in a later release)
- Role is `user` for JIT accounts (promote via Access Control)

See also [FEATURE_MATURITY.md](FEATURE_MATURITY.md), [SECURITY.md](../SECURITY.md), [UPGRADE.md](UPGRADE.md).
