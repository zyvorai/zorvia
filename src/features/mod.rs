//! Feature maturity registry and experimental gates.
//!
//! Levels match docs/FEATURE_MATURITY.md. Model-only and experimental
//! surfaces require an explicit env opt-in before they are advertised or
//! executed from operator-facing paths.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Maturity {
    /// Real backend, persistence, RBAC, E2E coverage, upgrade path.
    Ga,
    /// Operational with documented limitations.
    Beta,
    /// Behind a feature flag; no production promise.
    Experimental,
    /// Types/library only — not a shipping capability.
    ModelOnly,
}

impl Maturity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ga => "ga",
            Self::Beta => "beta",
            Self::Experimental => "experimental",
            Self::ModelOnly => "model-only",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Feature {
    pub id: &'static str,
    pub name: &'static str,
    pub maturity: Maturity,
    pub notes: &'static str,
}

/// Canonical registry (keep in sync with docs/FEATURE_MATURITY.md).
pub const FEATURES: &[Feature] = &[
    Feature {
        id: "vm-lifecycle",
        name: "VM create/start/stop/delete",
        maturity: Maturity::Ga,
        notes: "KubeVirt-backed; server RBAC enforced",
    },
    Feature {
        id: "snapshots-restore",
        name: "Snapshots & restore",
        maturity: Maturity::Ga,
        notes: "KubeVirt VirtualMachineSnapshot; empty-disk caveat documented",
    },
    Feature {
        id: "live-migration",
        name: "Live migration",
        maturity: Maturity::Ga,
        notes: "Requires shared storage / migration-capable cluster",
    },
    Feature {
        id: "web-console",
        name: "Web console + auth",
        maturity: Maturity::Ga,
        notes: "JWT + scoped API tokens; OIDC opt-in with PKCE/JWKS",
    },
    Feature {
        id: "audit-trail",
        name: "Audit trail",
        maturity: Maturity::Ga,
        notes: "SQLite persistence; GET /api/audit/export JSONL; optional ZORVIA_AUDIT_JSONL sidecar",
    },
    Feature {
        id: "pod-ops",
        name: "Pods: logs, exec, events, YAML, restart/delete",
        maturity: Maturity::Beta,
        notes: "cluster.admin only; needs pods delete + pods/log + pods/exec RBAC; exec/restart/delete audited; see docs/PODS.md",
    },
    Feature {
        id: "rescue-mode",
        name: "Rescue mode: hostname/SSH-key/enable-SSH via a Job-mounted disk",
        maturity: Maturity::Beta,
        notes: "cluster.admin only; needs batch/jobs RBAC; 3 of 5 UI operations wired (reset-password, install-packages, inspect deferred); see docs/RESCUE.md",
    },
    Feature {
        id: "schedulers",
        name: "Backup/power/alert/warm-pool schedulers",
        maturity: Maturity::Beta,
        notes: "Lease leader election; still in-process with API",
    },
    Feature {
        id: "rook-storage",
        name: "Rook-Ceph management",
        maturity: Maturity::Beta,
        notes: "Manages existing Ceph CRs; bootstrap SA optional",
    },
    Feature {
        id: "atlas-storage",
        name: "Atlas storage integration: inventory, backend/RBD/object-store, AI insights, observability, governance, DataBridge",
        maturity: Maturity::Beta,
        notes: "Optional, gated on ATLAS_URL; storage.admin; disaster recovery is scaffolding on Atlas's own side, unverified without a 2nd real Ceph cluster; see docs/ATLAS_INTEGRATION.md",
    },
    Feature {
        id: "helm-chart",
        name: "Helm production chart",
        maturity: Maturity::Beta,
        notes: "Lab + production values; GHCR digest pinning recommended",
    },
    Feature {
        id: "oidc",
        name: "OIDC / enterprise SSO",
        maturity: Maturity::Beta,
        notes: "Opt-in via ZORVIA_OIDC_ENABLED=1; PKCE + JWKS; see docs/OIDC_LAB.md",
    },
    Feature {
        id: "ai-troubleshoot",
        name: "AI troubleshooting wizard",
        maturity: Maturity::ModelOnly,
        notes: "Rules demo — not ML RCA; requires ZORVIA_EXPERIMENTAL=1",
    },
    Feature {
        id: "dr-replication",
        name: "DR replication pairs",
        maturity: Maturity::ModelOnly,
        notes: "In-memory model; does not move data",
    },
    Feature {
        id: "incremental-backup",
        name: "Incremental backup fields",
        maturity: Maturity::ModelOnly,
        notes: "Executions remain full VolumeSnapshots today",
    },
    Feature {
        id: "s3-immutable-backup",
        name: "Off-cluster S3 backups (encrypted, Object Lock), restore, recovery drills",
        maturity: Maturity::Experimental,
        notes: "Agent Job uploads checksummed, AES-GCM, Object-Lock copies with read-back verification; restore + drills (docs/OFFCLUSTER_BACKUP.md). Agent verified against a real S3 server; snapshot→Job flow not yet run on a snapshot-capable CSI. Plan API at /api/v1/enterprise/s3-backup/plan",
    },
    Feature {
        id: "cross-cluster-dr",
        name: "Cross-cluster VM mobility / DR",
        maturity: Maturity::Experimental,
        notes: "Phase 5 — plan API at /api/v1/enterprise/cross-cluster-dr/plan",
    },
    Feature {
        id: "transiva-migration",
        name: "VMware → KubeVirt via Transiva",
        maturity: Maturity::Experimental,
        notes: "Plan API only; superseded by vm-import-h2kvm (drives h2kvm directly, no Transiva). Plan API at /api/v1/enterprise/transiva/plan",
    },
    Feature {
        id: "vm-import-h2kvm",
        name: "VMware → KubeVirt import via h2kvm Jobs",
        maturity: Maturity::Experimental,
        notes: "POST /api/vm-imports runs h2kvmctl as a privileged Job (needs ZORVIA_H2KVM_IMAGE); admin console, fail-closed preflight, target NIC mapping, guarded cutover, boot policy, data-preserving cancellation; not yet exercised against a real vCenter",
    },
    Feature {
        id: "golden-image-pipeline",
        name: "Golden-image build/scan/sign/promote",
        maturity: Maturity::Experimental,
        notes: "Plan + POST /api/v1/enterprise/golden-pipeline/run applies CDI DV/DS; scan/sign deferred",
    },
    Feature {
        id: "commercial-contracts",
        name: "Commercial catalog, quote requests, contracts and coverage",
        maturity: Maturity::Beta,
        notes: "Contracts, coverage, support cases with contract-hours timers, redacted diagnostic bundles (preview/download; upload only on explicit confirm). Service engagements and managed-operation records (records only; no cluster access, remote operation off by default). Capacity reconciliation (unknown is never zero) and invoice records. Single-instance SQLite (ZORVIA_COMMERCIAL_DB); no notifications, payments, remote access or log collection; expiry never affects VMs — docs/COMMERCIAL_OFFERINGS.md",
    },
    Feature {
        id: "gpu-sriov-numa",
        name: "GPU/SR-IOV/NUMA placement",
        maturity: Maturity::Experimental,
        notes: "Phase 5 — plan API at /api/v1/enterprise/placement/gpu-numa",
    },
    Feature {
        id: "fleet-multicluster",
        name: "Multi-cluster fleet inventory",
        maturity: Maturity::Experimental,
        notes: "Live read-only inventory with explicit enrollment; GET /api/v1/enterprise/fleet; live multi-cluster validation pending",
    },
];

pub fn experimental_enabled() -> bool {
    match std::env::var("ZORVIA_EXPERIMENTAL") {
        Ok(v) => {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

pub fn feature(id: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.id == id)
}

/// Returns true if a model-only / experimental feature may be used.
pub fn allow_non_ga(id: &str) -> bool {
    match feature(id).map(|f| f.maturity) {
        Some(Maturity::Ga) | Some(Maturity::Beta) => true,
        Some(Maturity::Experimental) | Some(Maturity::ModelOnly) => experimental_enabled(),
        None => experimental_enabled(),
    }
}

/// Phase 5 enterprise feature toggles (all require experimental mode too).
pub fn enterprise_flag(env_key: &str) -> bool {
    experimental_enabled()
        && match std::env::var(env_key) {
            Ok(v) => {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true")
            }
            Err(_) => false,
        }
}

pub fn registry_json() -> serde_json::Value {
    serde_json::json!({
        "experimental_enabled": experimental_enabled(),
        "features": FEATURES.iter().map(|f| serde_json::json!({
            "id": f.id,
            "name": f.name,
            "maturity": f.maturity.as_str(),
            "notes": f.notes,
            "available": allow_non_ga(f.id),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ga_always_allowed() {
        assert!(allow_non_ga("vm-lifecycle"));
    }

    #[test]
    fn model_only_blocked_by_default() {
        // Do not set ZORVIA_EXPERIMENTAL in unit tests.
        if experimental_enabled() {
            return;
        }
        assert!(!allow_non_ga("ai-troubleshoot"));
        assert!(!allow_non_ga("dr-replication"));
    }
}
