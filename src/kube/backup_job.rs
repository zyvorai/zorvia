// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Builds the Kubernetes Job that runs `backup-agent` against a VM's disks:
//! each source PVC (restored from the VM snapshot) is mounted read-only under
//! `/disks/<name>/`, S3 settings arrive as plain env vars, and credentials and
//! the optional encryption key are injected from a Secret -- never placed in
//! the Job spec as values.

use anyhow::{bail, Result};
use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::{
    Container, EnvVar, EnvVarSource, PersistentVolumeClaimVolumeSource, PodSecurityContext,
    PodSpec, PodTemplateSpec, ResourceRequirements, SecretKeySelector, SecurityContext, Volume,
    VolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use std::collections::BTreeMap;

/// Where each disk's PVC is mounted; the agent reads `<mount>/disk.img`.
pub const DISKS_ROOT: &str = "/disks";
/// Where restore Jobs mount the (writable) target PVCs.
pub const RESTORE_ROOT: &str = "/restore";

pub fn backup_agent_image() -> String {
    std::env::var("ZORVIA_BACKUP_AGENT_IMAGE")
        .unwrap_or_else(|_| "ghcr.io/zyvorai/zorvia-backup-agent:latest".to_string())
}

/// UID that owns CDI-provisioned disk images (`qemu`).
fn agent_uid() -> i64 {
    std::env::var("ZORVIA_BACKUP_AGENT_UID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(107)
}

#[derive(Debug, Clone)]
pub struct BackupDisk {
    pub name: String,
    pub pvc: String,
}

/// Names of the Secret keys holding credentials.
#[derive(Debug, Clone)]
pub struct BackupSecretRef {
    pub secret_name: String,
    pub access_key_key: String,
    pub secret_key_key: String,
    /// Key holding the 64-hex-char encryption key, when encrypting.
    pub encryption_key_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BackupJobSpec {
    pub backup_id: String,
    pub vm_name: String,
    pub snapshot_name: String,
    pub disks: Vec<BackupDisk>,
    /// Non-secret `BACKUP_*` env (endpoint, bucket, prefix, lock mode, ...).
    pub env: Vec<(String, String)>,
    pub secrets: BackupSecretRef,
    pub active_deadline_secs: i64,
}

fn dns_label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

fn secret_env(name: &str, secret: &str, key: &str) -> EnvVar {
    EnvVar {
        name: name.to_string(),
        value_from: Some(EnvVarSource {
            secret_key_ref: Some(SecretKeySelector {
                name: secret.to_string(),
                key: key.to_string(),
                optional: Some(false),
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
enum Mode<'a> {
    Backup,
    Restore { manifest_key: &'a str },
}

pub fn build_backup_job(job_name: &str, namespace: &str, spec: &BackupJobSpec) -> Result<Job> {
    build_job(job_name, namespace, spec, Mode::Backup)
}

/// Job that reads a backup back onto freshly provisioned PVCs (`spec.disks`
/// are the *target* PVCs, mounted writable under `/restore/<name>/`).
pub fn build_restore_job(
    job_name: &str,
    namespace: &str,
    manifest_key: &str,
    spec: &BackupJobSpec,
) -> Result<Job> {
    if manifest_key.is_empty() || manifest_key.starts_with('/') || manifest_key.contains("..") {
        bail!("invalid manifest key");
    }
    build_job(job_name, namespace, spec, Mode::Restore { manifest_key })
}

fn build_job(job_name: &str, namespace: &str, spec: &BackupJobSpec, mode: Mode) -> Result<Job> {
    let (root, read_only) = match mode {
        Mode::Backup => (DISKS_ROOT, true),
        Mode::Restore { .. } => (RESTORE_ROOT, false),
    };
    if spec.disks.is_empty() {
        bail!("backup has no disks");
    }
    for d in &spec.disks {
        if !dns_label(&d.name) {
            bail!("invalid disk name '{}'", d.name);
        }
    }

    let disks_json = serde_json::to_string(
        &spec
            .disks
            .iter()
            .map(|d| {
                serde_json::json!({
                    "name": d.name,
                    "pvc": d.pvc,
                    "path": format!("{root}/{}/disk.img", d.name),
                })
            })
            .collect::<Vec<_>>(),
    )?;

    let mut env: Vec<EnvVar> = spec
        .env
        .iter()
        .map(|(k, v)| EnvVar {
            name: k.clone(),
            value: Some(v.clone()),
            ..Default::default()
        })
        .collect();
    for (k, v) in [
        ("BACKUP_ID", spec.backup_id.as_str()),
        ("BACKUP_VM", spec.vm_name.as_str()),
        ("BACKUP_NAMESPACE", namespace),
        ("BACKUP_SNAPSHOT", spec.snapshot_name.as_str()),
        (
            match mode {
                Mode::Backup => "BACKUP_DISKS",
                Mode::Restore { .. } => "RESTORE_DISKS",
            },
            disks_json.as_str(),
        ),
    ] {
        env.push(EnvVar {
            name: k.to_string(),
            value: Some(v.to_string()),
            ..Default::default()
        });
    }
    if let Mode::Restore { manifest_key } = mode {
        for (k, v) in [
            ("BACKUP_MODE", "restore"),
            ("RESTORE_MANIFEST_KEY", manifest_key),
        ] {
            env.push(EnvVar {
                name: k.to_string(),
                value: Some(v.to_string()),
                ..Default::default()
            });
        }
    }
    env.push(secret_env(
        "AWS_ACCESS_KEY_ID",
        &spec.secrets.secret_name,
        &spec.secrets.access_key_key,
    ));
    env.push(secret_env(
        "AWS_SECRET_ACCESS_KEY",
        &spec.secrets.secret_name,
        &spec.secrets.secret_key_key,
    ));
    if let Some(k) = &spec.secrets.encryption_key_key {
        env.push(secret_env(
            "BACKUP_ENCRYPTION_KEY",
            &spec.secrets.secret_name,
            k,
        ));
    }

    let mut labels = BTreeMap::new();
    labels.insert(
        "app.kubernetes.io/name".to_string(),
        match mode {
            Mode::Backup => "zorvia-backup",
            Mode::Restore { .. } => "zorvia-restore",
        }
        .to_string(),
    );
    labels.insert(
        "app.kubernetes.io/managed-by".to_string(),
        "zorvia".to_string(),
    );
    labels.insert("zorvia.io/backup-id".to_string(), spec.backup_id.clone());

    let mut requests = BTreeMap::new();
    requests.insert("cpu".to_string(), Quantity("250m".into()));
    // A part (default 64 MiB) is held in memory, plus its ciphertext copy.
    requests.insert("memory".to_string(), Quantity("512Mi".into()));
    let mut limits = BTreeMap::new();
    limits.insert("memory".to_string(), Quantity("1Gi".into()));

    Ok(Job {
        metadata: ObjectMeta {
            name: Some(job_name.to_string()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(JobSpec {
            // The agent aborts its multipart upload on failure and refuses to
            // overwrite an existing backup, so a retry starts clean.
            // A restore refuses to overwrite files, so a pod-level retry
            // could never succeed; the operation layer decides on retries.
            backoff_limit: Some(match mode {
                Mode::Backup => 2,
                Mode::Restore { .. } => 0,
            }),
            ttl_seconds_after_finished: Some(3600),
            active_deadline_seconds: Some(spec.active_deadline_secs),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    restart_policy: Some("Never".to_string()),
                    security_context: Some(PodSecurityContext {
                        run_as_user: Some(agent_uid()),
                        run_as_non_root: Some(true),
                        fs_group: Some(agent_uid()),
                        ..Default::default()
                    }),
                    containers: vec![Container {
                        name: "backup-agent".to_string(),
                        image: Some(backup_agent_image()),
                        env: Some(env),
                        resources: Some(ResourceRequirements {
                            requests: Some(requests),
                            limits: Some(limits),
                            ..Default::default()
                        }),
                        security_context: Some(SecurityContext {
                            privileged: Some(false),
                            allow_privilege_escalation: Some(false),
                            read_only_root_filesystem: Some(true),
                            capabilities: Some(k8s_openapi::api::core::v1::Capabilities {
                                drop: Some(vec!["ALL".to_string()]),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        volume_mounts: Some(
                            spec.disks
                                .iter()
                                .map(|d| VolumeMount {
                                    name: format!("disk-{}", d.name),
                                    mount_path: format!("{root}/{}", d.name),
                                    read_only: Some(read_only),
                                    ..Default::default()
                                })
                                .collect(),
                        ),
                        ..Default::default()
                    }],
                    volumes: Some(
                        spec.disks
                            .iter()
                            .map(|d| Volume {
                                name: format!("disk-{}", d.name),
                                persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                                    claim_name: d.pvc.clone(),
                                    read_only: Some(read_only),
                                }),
                                ..Default::default()
                            })
                            .collect(),
                    ),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        status: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> BackupJobSpec {
        BackupJobSpec {
            backup_id: "bk-1".into(),
            vm_name: "web-1".into(),
            snapshot_name: "backup-web-1-abc".into(),
            disks: vec![
                BackupDisk {
                    name: "root".into(),
                    pvc: "bk-1-root".into(),
                },
                BackupDisk {
                    name: "data".into(),
                    pvc: "bk-1-data".into(),
                },
            ],
            env: vec![("BACKUP_S3_BUCKET".into(), "vm-backups".into())],
            secrets: BackupSecretRef {
                secret_name: "backup-s3".into(),
                access_key_key: "access-key".into(),
                secret_key_key: "secret-key".into(),
                encryption_key_key: Some("enc-key".into()),
            },
            active_deadline_secs: 21600,
        }
    }

    fn pod(job: &Job) -> &PodSpec {
        job.spec.as_ref().unwrap().template.spec.as_ref().unwrap()
    }

    #[test]
    fn mounts_every_disk_read_only_under_disks_root() {
        let job = build_backup_job("bk-1", "default", &spec()).unwrap();
        let c = &pod(&job).containers[0];
        let mounts = c.volume_mounts.as_ref().unwrap();
        assert_eq!(mounts.len(), 2);
        assert!(mounts.iter().all(|m| m.read_only == Some(true)));
        assert_eq!(mounts[0].mount_path, "/disks/root");
        let vols = pod(&job).volumes.as_ref().unwrap();
        assert_eq!(
            vols[1].persistent_volume_claim.as_ref().unwrap().claim_name,
            "bk-1-data"
        );
        assert_eq!(
            vols[1].persistent_volume_claim.as_ref().unwrap().read_only,
            Some(true)
        );
    }

    #[test]
    fn credentials_come_from_the_secret_never_as_values() {
        let job = build_backup_job("bk-1", "default", &spec()).unwrap();
        let env = pod(&job).containers[0].env.as_ref().unwrap();
        for name in [
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "BACKUP_ENCRYPTION_KEY",
        ] {
            let e = env.iter().find(|e| e.name == name).unwrap();
            assert!(e.value.is_none(), "{name} must not be a literal value");
            assert_eq!(
                e.value_from
                    .as_ref()
                    .unwrap()
                    .secret_key_ref
                    .as_ref()
                    .unwrap()
                    .name,
                "backup-s3"
            );
        }
        let disks = env.iter().find(|e| e.name == "BACKUP_DISKS").unwrap();
        assert!(disks
            .value
            .as_ref()
            .unwrap()
            .contains("/disks/root/disk.img"));
    }

    #[test]
    fn container_is_locked_down() {
        let job = build_backup_job("bk-1", "default", &spec()).unwrap();
        let sc = pod(&job).containers[0].security_context.as_ref().unwrap();
        assert_eq!(sc.privileged, Some(false));
        assert_eq!(sc.allow_privilege_escalation, Some(false));
        assert_eq!(sc.read_only_root_filesystem, Some(true));
        assert_eq!(
            sc.capabilities.as_ref().unwrap().drop,
            Some(vec!["ALL".to_string()])
        );
        assert_eq!(
            pod(&job).security_context.as_ref().unwrap().run_as_non_root,
            Some(true)
        );
    }

    #[test]
    fn omits_encryption_env_when_not_encrypting() {
        let mut s = spec();
        s.secrets.encryption_key_key = None;
        let job = build_backup_job("bk-1", "default", &s).unwrap();
        let env = pod(&job).containers[0].env.as_ref().unwrap();
        assert!(!env.iter().any(|e| e.name == "BACKUP_ENCRYPTION_KEY"));
    }

    #[test]
    fn restore_job_mounts_targets_writable_and_selects_restore_mode() {
        let job =
            build_restore_job("rs-1", "default", "web-1/op-1/manifest.json", &spec()).unwrap();
        let c = &pod(&job).containers[0];
        let mounts = c.volume_mounts.as_ref().unwrap();
        assert_eq!(mounts[0].mount_path, "/restore/root");
        assert!(mounts.iter().all(|m| m.read_only == Some(false)));
        let vols = pod(&job).volumes.as_ref().unwrap();
        assert_eq!(
            vols[0].persistent_volume_claim.as_ref().unwrap().read_only,
            Some(false)
        );
        let env = c.env.as_ref().unwrap();
        let val = |n: &str| {
            env.iter()
                .find(|e| e.name == n)
                .and_then(|e| e.value.clone())
        };
        assert_eq!(val("BACKUP_MODE").as_deref(), Some("restore"));
        assert_eq!(
            val("RESTORE_MANIFEST_KEY").as_deref(),
            Some("web-1/op-1/manifest.json")
        );
        assert!(val("RESTORE_DISKS")
            .unwrap()
            .contains("/restore/root/disk.img"));
        assert!(val("BACKUP_DISKS").is_none());
        assert_eq!(job.spec.as_ref().unwrap().backoff_limit, Some(0));
        // Still no privileges, and credentials still come from the Secret.
        let sc = c.security_context.as_ref().unwrap();
        assert_eq!(sc.privileged, Some(false));
        assert!(env
            .iter()
            .find(|e| e.name == "AWS_SECRET_ACCESS_KEY")
            .unwrap()
            .value
            .is_none());
    }

    #[test]
    fn restore_job_rejects_bad_manifest_keys() {
        for k in ["", "/abs", "a/../b"] {
            assert!(build_restore_job("rs-1", "default", k, &spec()).is_err());
        }
    }

    #[test]
    fn rejects_bad_disks() {
        let mut s = spec();
        s.disks[0].name = "Bad/Name".into();
        assert!(build_backup_job("bk-1", "default", &s).is_err());
        s.disks.clear();
        assert!(build_backup_job("bk-1", "default", &s).is_err());
    }
}
