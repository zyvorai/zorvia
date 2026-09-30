# Diagnostic privacy

> **Status: not implemented.** No diagnostic bundle is produced or uploaded by this release. This page is the privacy contract the feature must meet before it ships.

## A bundle will contain

Zorvia, Kubernetes, KubeVirt and CDI versions; sanitized VM specifications and recent events; relevant storage and scheduling conditions; operation failures and selected application logs; component health and configuration summaries.

## A bundle will never contain

Credentials, Secret contents, kubeconfigs, tokens, cloud-init user data, guest disks or memory dumps.

## Handling rules

- Free-form log text is redacted for sensitive values before it is written.
- An administrator previews the exact contents before export.
- Default is a **local download**. Uploading to support is a separate, explicit action.
- Attachment size limits and retention are configurable.
- Access is isolated by customer organization.
