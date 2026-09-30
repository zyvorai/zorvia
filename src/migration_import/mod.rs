//! VMware -> KubeVirt import driven from Zorvia by running `h2kvmctl` as a
//! Kubernetes Job (h2kvm is an external, separately licensed tool: Zorvia only
//! invokes its container image, it never links it).
//!
//! One `vm-import` operation per source VM; a *wave* is a batch of them queued
//! together and run under a concurrency cap. `h2kvmctl --cmd vsphere ...
//! --deploy-k8s` does the whole pipeline (export from vCenter, GuestKit
//! offline repair, convert, upload via CDI, create the VirtualMachine); Zorvia
//! supplies parameters, credentials (from a Secret), scratch space, follows the
//! stage markers h2kvm logs, verifies the VM exists, and rolls back on failure.

pub mod job;
pub mod runtime;

use crate::operations::{NewOperation, Operation, OperationsDb};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const OP_KIND: &str = "vm-import";
pub const MAX_WAVE: usize = 50;

/// Where the source VMs live. The password never appears here: it is read by
/// the Job from `secret_name` in the target namespace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VsphereSource {
    pub vcenter: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub datacenter: Option<String>,
    #[serde(default)]
    pub insecure: bool,
    /// Secret (in the target namespace) holding the vCenter credentials.
    pub secret_name: String,
    #[serde(default)]
    pub username_key: Option<String>,
    #[serde(default)]
    pub password_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportVm {
    /// VM name as vCenter knows it.
    pub source_vm: String,
    /// Defaults to a DNS-label form of `source_vm`.
    #[serde(default)]
    pub target_vm_name: Option<String>,
    #[serde(default)]
    pub storage_class: Option<String>,
    /// Size of the root PVC, e.g. "60Gi" (h2kvm detects it when omitted).
    #[serde(default)]
    pub pvc_size: Option<String>,
    #[serde(default)]
    pub cpu: Option<u32>,
    /// e.g. "8Gi".
    #[serde(default)]
    pub memory: Option<String>,
    /// Start the VM once deployed (cutover). Default: leave it stopped.
    #[serde(default)]
    pub start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportRequest {
    pub source: VsphereSource,
    /// Namespace the VMs are created in (and the Job runs in).
    pub namespace: String,
    pub vms: Vec<ImportVm>,
    /// Scratch space for the exported disk and the converted image. Must fit
    /// the largest disk twice over. Default 200Gi.
    #[serde(default)]
    pub scratch_size: Option<String>,
    #[serde(default)]
    pub scratch_storage_class: Option<String>,
    /// ServiceAccount the Job runs as (needs rights to create VMs, PVCs and
    /// DataVolumes in the namespace). Default `zorvia-h2kvm`.
    #[serde(default)]
    pub service_account: Option<String>,
}

/// What one operation persists. No secret values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOpParams {
    pub wave_id: String,
    pub source: VsphereSource,
    pub namespace: String,
    pub vm: ImportVm,
    pub target_vm_name: String,
    pub scratch_size: String,
    pub scratch_storage_class: Option<String>,
    pub service_account: String,
}

pub fn dns_label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// A value that becomes one argv element of `h2kvmctl`: not a shell string,
/// but must not start with `-` (option injection) or contain control chars.
fn safe_arg(what: &str, s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 256 {
        bail!("{what} must be 1-256 characters");
    }
    if s.starts_with('-') {
        bail!("{what} must not start with '-'");
    }
    if s.chars().any(|c| c.is_control()) {
        bail!("{what} contains control characters");
    }
    Ok(())
}

fn quantity(what: &str, s: &str) -> Result<()> {
    let ok = !s.is_empty() && s.len() <= 16 && {
        let digits: String = s
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let suffix = &s[digits.len()..];
        !digits.is_empty()
            && digits.matches('.').count() <= 1
            && matches!(
                suffix,
                "" | "Ki" | "Mi" | "Gi" | "Ti" | "K" | "M" | "G" | "T"
            )
    };
    if !ok {
        bail!("{what} must be a Kubernetes quantity like 60Gi");
    }
    Ok(())
}

fn hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && !s.starts_with(['-', '.'])
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
}

/// DNS-label form of a vCenter VM name ("Web Prod_01" -> "web-prod-01").
pub fn default_target_name(source_vm: &str) -> String {
    let mut out: String = source_vm
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    let trimmed = out.trim_matches('-');
    let mut name: String = trimmed.chars().take(63).collect();
    name = name.trim_end_matches('-').to_string();
    if name.is_empty() {
        "imported-vm".to_string()
    } else {
        name
    }
}

fn note(p: &mut Vec<String>, r: Result<()>) {
    if let Err(e) = r {
        p.push(e.to_string());
    }
}

impl ImportRequest {
    /// Every problem with the request (empty when valid).
    pub fn problems(&self) -> Vec<String> {
        let mut p: Vec<String> = Vec::new();
        if !hostname(&self.source.vcenter) {
            p.push("source.vcenter is not a valid host name or address".into());
        }
        if !dns_label(&self.namespace) {
            p.push("namespace must be a DNS label".into());
        }
        if !dns_label(&self.source.secret_name) {
            p.push("source.secret_name must be a DNS label".into());
        }
        for (k, v) in [
            ("source.username_key", &self.source.username_key),
            ("source.password_key", &self.source.password_key),
            ("source.datacenter", &self.source.datacenter),
        ] {
            if let Some(v) = v {
                note(&mut p, safe_arg(k, v));
            }
        }
        if let Some(sa) = &self.service_account {
            if !dns_label(sa) {
                p.push("service_account must be a DNS label".into());
            }
        }
        if let Some(s) = &self.scratch_size {
            note(&mut p, quantity("scratch_size", s));
        }
        if self.vms.is_empty() {
            p.push("vms is empty".into());
        }
        if self.vms.len() > MAX_WAVE {
            p.push(format!("a wave holds at most {MAX_WAVE} VMs"));
        }
        let mut names = std::collections::HashSet::new();
        for (i, vm) in self.vms.iter().enumerate() {
            let at = format!("vms[{i}]");
            note(&mut p, safe_arg(&format!("{at}.source_vm"), &vm.source_vm));
            let target = vm
                .target_vm_name
                .clone()
                .unwrap_or_else(|| default_target_name(&vm.source_vm));
            if !dns_label(&target) {
                p.push(format!("{at}.target_vm_name must be a DNS label"));
            } else if !names.insert(target.clone()) {
                p.push(format!(
                    "{at}: target name '{target}' is used twice in this wave"
                ));
            }
            if let Some(sc) = &vm.storage_class {
                note(&mut p, safe_arg(&format!("{at}.storage_class"), sc));
            }
            if let Some(s) = &vm.pvc_size {
                note(&mut p, quantity(&format!("{at}.pvc_size"), s));
            }
            if let Some(m) = &vm.memory {
                note(&mut p, quantity(&format!("{at}.memory"), m));
            }
            if let Some(c) = vm.cpu {
                if c == 0 || c > 256 {
                    p.push(format!("{at}.cpu must be 1-256"));
                }
            }
        }
        p
    }

    pub fn validate(&self) -> Result<()> {
        let p = self.problems();
        if p.is_empty() {
            Ok(())
        } else {
            bail!("{}", p.join("; "))
        }
    }
}

/// argv for `h2kvmctl` (after the binary). `$(VC_USER)` is expanded by
/// Kubernetes from the Job's env, so the credentials never enter the spec.
pub fn h2kvm_args(p: &ImportOpParams) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--cmd".into(),
        "vsphere".into(),
        "--vcenter".into(),
        p.source.vcenter.clone(),
        "--vc-user".into(),
        "$(VC_USER)".into(),
        "--vc-password-env".into(),
        "VC_PASSWORD".into(),
        "--vs-vm".into(),
        p.vm.source_vm.clone(),
        "--output-dir".into(),
        "/work/out".into(),
        "--to-output".into(),
        format!("{}.qcow2", p.target_vm_name),
        "--flatten".into(),
        "--deploy-k8s".into(),
        "--k8s-namespace".into(),
        p.namespace.clone(),
        "--k8s-vm-name".into(),
        p.target_vm_name.clone(),
        "--k8s-pvc-name".into(),
        pvc_name(&p.target_vm_name),
    ];
    if let Some(port) = p.source.port {
        a.extend(["--vc-port".into(), port.to_string()]);
    }
    if p.source.insecure {
        a.push("--vc-insecure".into());
    }
    if let Some(dc) = &p.source.datacenter {
        a.extend(["--dc-name".into(), dc.clone()]);
    }
    if let Some(sc) = &p.vm.storage_class {
        a.extend(["--k8s-storage-class".into(), sc.clone()]);
    }
    if let Some(sz) = &p.vm.pvc_size {
        a.extend(["--k8s-pvc-size".into(), sz.clone()]);
    }
    if let Some(cpu) = p.vm.cpu {
        a.extend(["--k8s-cpu".into(), cpu.to_string()]);
    }
    if let Some(mem) = &p.vm.memory {
        a.extend(["--k8s-memory".into(), mem.clone()]);
    }
    if p.vm.start {
        a.push("--k8s-auto-start".into());
    }
    a
}

/// Root PVC name for an imported VM (deterministic, so rollback can find it).
pub fn pvc_name(target_vm: &str) -> String {
    let mut n = format!("{target_vm}-root");
    n.truncate(63);
    n
}

/// What we can tell from h2kvm's log output.
#[derive(Debug, Default, PartialEq)]
pub struct H2kvmProgress {
    pub current_stage: Option<String>,
    pub stages_started: u32,
    pub stages_done: u32,
    pub deployed_vm: Option<String>,
    pub failure: Option<String>,
}

impl H2kvmProgress {
    /// Rough percentage: h2kvm reports stage boundaries, not bytes. Capped at
    /// 95 until the VM is confirmed deployed.
    pub fn percent(&self) -> u8 {
        if self.deployed_vm.is_some() {
            return 95;
        }
        // Typical run: inspect, export/flatten, fix, convert, validate,
        // upload, deploy... treat 8 stages as the expected total.
        (self.stages_done * 100 / 8).min(90) as u8
    }
}

/// Parse h2kvm log lines (`➡️  Stage: X`, `✅ x completed in Ns`,
/// `✅ Deployed: <vm>`, `❌ x failed: reason`).
pub fn parse_h2kvm_logs(logs: &str) -> H2kvmProgress {
    let mut p = H2kvmProgress::default();
    for raw in logs.lines() {
        let line = raw.trim();
        if let Some(rest) = line.split("Stage:").nth(1).filter(|_| line.contains('➡')) {
            p.stages_started += 1;
            p.current_stage = Some(rest.trim().to_ascii_lowercase());
        } else if line.contains('✅') && line.contains("completed in") {
            p.stages_done += 1;
        } else if let Some(rest) = line
            .split("Deployed:")
            .nth(1)
            .filter(|_| line.contains('✅'))
        {
            let vm = rest.trim();
            if !vm.is_empty() {
                p.deployed_vm = Some(vm.to_string());
            }
        } else if line.contains('❌') {
            let msg: String = line.chars().filter(|c| *c != '❌').collect();
            p.failure = Some(msg.trim().chars().take(500).collect());
        }
    }
    p
}

fn short(id: &str) -> String {
    id.trim_start_matches("op-").chars().take(10).collect()
}

/// Queue one `vm-import` operation per VM under a new wave id.
pub fn enqueue_wave_in(db: &OperationsDb, req: &ImportRequest) -> Result<(String, Vec<Operation>)> {
    req.validate()?;
    let wave_id = format!("wave-{}", short(&uuid::Uuid::new_v4().simple().to_string()));
    let active = db.list_active()?;
    let mut ops = Vec::new();
    for vm in &req.vms {
        let target = vm
            .target_vm_name
            .clone()
            .unwrap_or_else(|| default_target_name(&vm.source_vm));
        if active.iter().any(|o| {
            o.kind == OP_KIND
                && o.namespace == req.namespace
                && o.params.get("target_vm_name").and_then(|t| t.as_str()) == Some(target.as_str())
        }) {
            bail!("an import to '{target}' is already in progress");
        }
        let params = ImportOpParams {
            wave_id: wave_id.clone(),
            source: req.source.clone(),
            namespace: req.namespace.clone(),
            vm: vm.clone(),
            target_vm_name: target.clone(),
            scratch_size: req.scratch_size.clone().unwrap_or_else(|| "200Gi".into()),
            scratch_storage_class: req.scratch_storage_class.clone(),
            service_account: req
                .service_account
                .clone()
                .unwrap_or_else(|| "zorvia-h2kvm".into()),
        };
        let mut new = NewOperation::new(OP_KIND, &vm.source_vm, &req.namespace);
        new.max_attempts = 1; // an import is not safely repeatable; the user retries
        new.params = serde_json::to_value(&params)?;
        ops.push(db.create(new)?.0);
    }
    Ok((wave_id, ops))
}

/// Counts by state for every operation of a wave.
pub fn wave_summary(db: &OperationsDb, wave_id: &str) -> Result<serde_json::Value> {
    let ops: Vec<Operation> = db
        .list(Some(OP_KIND), 500)?
        .into_iter()
        .filter(|o| o.params.get("wave_id").and_then(|w| w.as_str()) == Some(wave_id))
        .collect();
    let count = |s: crate::operations::OpState| ops.iter().filter(|o| o.state == s).count();
    use crate::operations::OpState::*;
    Ok(serde_json::json!({
        "wave_id": wave_id,
        "total": ops.len(),
        "queued": count(Queued),
        "running": count(Running),
        "succeeded": count(Succeeded),
        "failed": count(Failed),
        "cancelled": count(Cancelled),
        "operations": ops.iter().map(|o| serde_json::json!({
            "id": o.id,
            "source_vm": o.resource,
            "target_vm_name": o.params.get("target_vm_name"),
            "state": o.state,
            "phase": o.phase,
            "progress": o.progress,
            "error": o.error,
        })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> ImportRequest {
        ImportRequest {
            source: VsphereSource {
                vcenter: "vc.example.com".into(),
                port: None,
                datacenter: Some("DC1".into()),
                insecure: false,
                secret_name: "vcenter-creds".into(),
                username_key: None,
                password_key: None,
            },
            namespace: "vms".into(),
            vms: vec![ImportVm {
                source_vm: "Web Prod_01".into(),
                target_vm_name: None,
                storage_class: Some("longhorn".into()),
                pvc_size: Some("60Gi".into()),
                cpu: Some(4),
                memory: Some("8Gi".into()),
                start: true,
            }],
            scratch_size: None,
            scratch_storage_class: None,
            service_account: None,
        }
    }

    #[test]
    fn valid_request_passes_and_names_are_derived() {
        assert!(req().problems().is_empty());
        assert_eq!(default_target_name("Web Prod_01"), "web-prod-01");
        assert_eq!(default_target_name("--Bad//Name--"), "bad-name");
        assert_eq!(default_target_name("***"), "imported-vm");
        assert!(dns_label(&default_target_name(&"x".repeat(200))));
    }

    #[test]
    fn rejects_option_injection_and_bad_values() {
        let mut r = req();
        r.vms[0].source_vm = "--vc-insecure".into();
        assert!(r.validate().is_err());
        let mut r = req();
        r.source.vcenter = "vc.example.com --deploy-openstack".into();
        assert!(r.validate().is_err());
        let mut r = req();
        r.vms[0].pvc_size = Some("60GB; rm".into());
        assert!(r.validate().is_err());
        let mut r = req();
        r.vms[0].memory = Some("-1Gi".into());
        assert!(r.validate().is_err());
        let mut r = req();
        r.vms[0].source_vm = "vm\nname".into();
        assert!(r.validate().is_err());
        let mut r = req();
        r.vms[0].cpu = Some(0);
        assert!(r.validate().is_err());
        let mut r = req();
        r.namespace = "Bad_NS".into();
        assert!(r.validate().is_err());
        let mut r = req();
        r.vms.clear();
        assert!(r.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_targets_within_a_wave() {
        let mut r = req();
        let mut second = r.vms[0].clone();
        second.source_vm = "web prod 01".into(); // same derived name
        r.vms.push(second);
        assert!(r.problems().iter().any(|p| p.contains("used twice")));
    }

    fn op_params() -> ImportOpParams {
        let r = req();
        ImportOpParams {
            wave_id: "wave-1".into(),
            source: r.source.clone(),
            namespace: "vms".into(),
            vm: r.vms[0].clone(),
            target_vm_name: "web-prod-01".into(),
            scratch_size: "200Gi".into(),
            scratch_storage_class: None,
            service_account: "zorvia-h2kvm".into(),
        }
    }

    #[test]
    fn builds_h2kvmctl_args_without_credentials() {
        let a = h2kvm_args(&op_params());
        let joined = a.join(" ");
        assert!(joined.starts_with("--cmd vsphere --vcenter vc.example.com"));
        assert!(joined.contains("--vc-user $(VC_USER) --vc-password-env VC_PASSWORD"));
        assert!(joined.contains("--vs-vm Web Prod_01"));
        assert!(joined.contains("--deploy-k8s --k8s-namespace vms --k8s-vm-name web-prod-01"));
        assert!(joined.contains("--k8s-pvc-name web-prod-01-root"));
        assert!(joined.contains("--dc-name DC1"));
        assert!(joined.contains("--k8s-storage-class longhorn --k8s-pvc-size 60Gi"));
        assert!(joined.contains("--k8s-cpu 4 --k8s-memory 8Gi"));
        assert!(a.contains(&"--k8s-auto-start".to_string()));
        assert!(!a.contains(&"--vc-insecure".to_string()));
        // The VM name is a single argv element, never split on spaces.
        assert!(a.iter().any(|x| x == "Web Prod_01"));
        let mut p = op_params();
        p.source.insecure = true;
        p.source.port = Some(8443);
        p.vm.start = false;
        let a = h2kvm_args(&p);
        assert!(a.contains(&"--vc-insecure".to_string()));
        assert!(a.windows(2).any(|w| w == ["--vc-port", "8443"]));
        assert!(!a.contains(&"--k8s-auto-start".to_string()));
    }

    #[test]
    fn parses_stage_markers_deploy_and_failure() {
        let logs = "starting\n➡️  Stage: INSPECT\n✅ inspect completed in 1.20s\n\
                    ➡️  Stage: FIX\n✅ fix completed in 30.00s\n➡️  Stage: CONVERT\n";
        let p = parse_h2kvm_logs(logs);
        assert_eq!((p.stages_started, p.stages_done), (3, 2));
        assert_eq!(p.current_stage.as_deref(), Some("convert"));
        assert_eq!(p.percent(), 25);
        assert!(p.deployed_vm.is_none() && p.failure.is_none());

        let done = format!("{logs}✅ Deployed: web-prod-01\n   Namespace: vms\n");
        let p = parse_h2kvm_logs(&done);
        assert_eq!(p.deployed_vm.as_deref(), Some("web-prod-01"));
        assert_eq!(p.percent(), 95);

        let failed = "➡️  Stage: FIX\n❌ fix failed: guest has no /etc/fstab\n";
        let p = parse_h2kvm_logs(failed);
        assert_eq!(
            p.failure.as_deref(),
            Some("fix failed: guest has no /etc/fstab")
        );
        assert_eq!(
            parse_h2kvm_logs("nothing useful\n"),
            H2kvmProgress::default()
        );
    }

    #[test]
    fn waves_queue_one_operation_per_vm_and_block_duplicates() {
        let db = OperationsDb::open(":memory:").unwrap();
        let mut r = req();
        let mut second = r.vms[0].clone();
        second.source_vm = "db-01".into();
        r.vms.push(second);
        let (wave, ops) = enqueue_wave_in(&db, &r).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(ops.iter().all(|o| o.kind == OP_KIND && o.max_attempts == 1));
        assert_eq!(ops[0].params["wave_id"], wave.as_str());
        assert_eq!(ops[0].params["scratch_size"], "200Gi");
        // Credentials are never persisted, only the Secret name.
        assert!(!ops[0].params.to_string().contains("\"password\":"));
        assert!(
            enqueue_wave_in(&db, &r).is_err(),
            "same targets already queued"
        );

        let s = wave_summary(&db, &wave).unwrap();
        assert_eq!(s["total"], 2);
        assert_eq!(s["queued"], 2);
        // Newest first; order-independent check.
        let targets: Vec<&str> = s["operations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|o| o["target_vm_name"].as_str())
            .collect();
        assert!(targets.contains(&"db-01") && targets.contains(&"web-prod-01"));
    }
}
