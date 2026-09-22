# Security Policy

## Supported Versions

We release patches for security vulnerabilities for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.3.x   | :white_check_mark: |
| 0.2.x   | :x:                |
| 0.1.x   | :x:                |

**0.3.3** hardens authentication and authorization (server-side RBAC, scoped API
tokens, JWT revocation, optional secure OIDC (`ZORVIA_OIDC_ENABLED=1`), lab
credential guards, webhook SSRF hardening). Upgrade from 0.3.2 as soon as
practical.

## Reporting a Vulnerability

We take the security of Zorvia seriously. If you have discovered a security vulnerability, please follow these steps:

### Private Disclosure

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please report them via one of the following methods:

1. **GitHub Security Advisories** (Preferred)
   - Go to the [Security tab](https://github.com/zyvorai/zorvia/security/advisories)
   - Click "Report a vulnerability"
   - Fill in the details

2. **Email**
   - Send an email to: info@zyvor.dev
   - Include as much information as possible (see below)

### What to Include

Please include the following information:

- **Type of vulnerability** (e.g. authorization bypass, SSRF, credential exposure)
- **Full paths of source file(s)** related to the vulnerability
- **Location of the affected source code** (tag/branch/commit or direct URL)
- **Step-by-step instructions to reproduce** the issue
- **Proof-of-concept or exploit code** (if possible)
- **Impact of the issue**, including how an attacker might exploit it

### Response Timeline

- We aim to acknowledge reports within **72 hours**
- We aim to provide an initial assessment within **7 days**
- Critical issues will be prioritized for immediate patching

### Disclosure Policy

- We request that you give us reasonable time to address the issue before public disclosure
- We will credit researchers who responsibly disclose vulnerabilities (unless anonymity is requested)
- Once a fix is available, we will publish a security advisory with details

## Security Best Practices

When deploying Zorvia:

1. **Do not use lab defaults in production** — omit `ZORVIA_LAB_MODE`, set unique
   `ZORVIA_JWT_SECRET` and `ZORVIA_ADMIN_PASSWORD`, and create auth Secrets via
   `./scripts/create-auth-secret.sh` (never commit Secret manifests).
2. Prefer **scoped API tokens** (`POST /api/v1/api-tokens`) over a shared
   `ZORVIA_API_KEY` (lab mode only).
3. Use TLS in production (Ingress/cert-manager preferred over NodePort self-signed).
4. Keep dependencies updated (`cargo update`, Dependabot/Renovate).
5. Run Zorvia with least-privilege Kubernetes RBAC — apply
   `deploy/rook-bootstrap-rbac.yaml` only when intentionally bootstrapping Rook.
6. Restrict webhook destinations with `ZORVIA_WEBHOOK_ALLOWLIST` when possible.
7. **OIDC is opt-in** — set `ZORVIA_OIDC_ENABLED=1` with issuer/client/secret/redirect
   for PKCE + token exchange + JWKS verification. Without the enable flag, OIDC
   env vars are ignored.

## Known Security Considerations

- JWT access tokens default to a **60-minute** TTL (`ZORVIA_JWT_TTL_MINUTES`);
  local-user disable/role/password/TOTP changes bump `token_version` and revoke
  outstanding sessions.
- TOTP disable requires password + current TOTP code and revokes sessions.
- The shared env API key is ignored unless `ZORVIA_LAB_MODE=1`.
- Webhook delivery resolves DNS, pins the address, and re-validates redirects;
  private/link-local destinations are rejected.

## Contact

For security-related inquiries: info@zyvor.dev
