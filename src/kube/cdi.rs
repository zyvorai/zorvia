//! CDI DataVolume helpers for data-preserving PVC clones and image imports.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::kube::lifecycle::validate_k8s_name;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CdiPvcCloneSpec {
    pub source_namespace: String,
    pub source_pvc: String,
    pub target_namespace: String,
    pub target_name: String,
    pub size: String,
    pub storage_class: Option<String>,
}

impl CdiPvcCloneSpec {
    pub fn validate(&self) -> Result<()> {
        validate_k8s_name("source namespace", &self.source_namespace)?;
        validate_k8s_name("source PVC", &self.source_pvc)?;
        validate_k8s_name("target namespace", &self.target_namespace)?;
        validate_k8s_name("target DataVolume", &self.target_name)?;
        if self.size.trim().is_empty() {
            bail!("clone size must not be empty");
        }
        Ok(())
    }
}

/// CDI DataVolume that clones an existing PVC (bit-preserving when CDI is installed).
pub fn data_volume_clone_manifest(spec: &CdiPvcCloneSpec) -> Result<Value> {
    spec.validate()?;
    let mut storage = json!({
        "resources": { "requests": { "storage": spec.size } }
    });
    if let Some(sc) = &spec.storage_class {
        storage["storageClassName"] = json!(sc);
    }
    Ok(json!({
        "apiVersion": "cdi.kubevirt.io/v1beta1",
        "kind": "DataVolume",
        "metadata": {
            "name": spec.target_name,
            "namespace": spec.target_namespace,
            "labels": {
                "app.kubernetes.io/managed-by": "zorvia",
                "zorvia.io/clone-source": spec.source_pvc,
            }
        },
        "spec": {
            "source": {
                "pvc": {
                    "namespace": spec.source_namespace,
                    "name": spec.source_pvc
                }
            },
            "storage": storage
        }
    }))
}

/// Suggested DataVolume name for a cloned disk.
pub fn clone_dv_name(target_vm: &str, disk: &str) -> String {
    let raw = format!("{target_vm}-{disk}");
    raw.chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
                c
            } else if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(63)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> CdiPvcCloneSpec {
        CdiPvcCloneSpec {
            source_namespace: "default".into(),
            source_pvc: "prod-db-root".into(),
            target_namespace: "default".into(),
            target_name: "staging-db-root".into(),
            size: "40Gi".into(),
            storage_class: Some("fast".into()),
        }
    }

    #[test]
    fn clone_manifest_points_at_source_pvc() {
        let dv = data_volume_clone_manifest(&spec()).unwrap();
        assert_eq!(dv["kind"], "DataVolume");
        assert_eq!(dv["spec"]["source"]["pvc"]["name"], "prod-db-root");
        assert_eq!(dv["spec"]["storage"]["storageClassName"], "fast");
        assert_eq!(
            dv["spec"]["storage"]["resources"]["requests"]["storage"],
            "40Gi"
        );
    }

    #[test]
    fn rejects_bad_names() {
        let mut bad = spec();
        bad.target_name = "NOPE_UPPER".into();
        assert!(data_volume_clone_manifest(&bad).is_err());
    }

    #[test]
    fn dv_name_is_dns_safe() {
        let name = clone_dv_name("Web_01", "root disk");
        assert!(name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        assert!(name.len() <= 63);
    }
}
