//! The `vm-import` operation: run the h2kvm Job, follow its stage markers,
//! verify the VM exists, optionally confirm it boots, and roll back anything
//! this import created if it fails or is cancelled.

use super::job::{build_import_job, h2kvm_image};
use super::{parse_h2kvm_logs, pvc_name, H2kvmProgress, ImportOpParams};
use crate::backup::offcluster::{job_state, pod_config_error, JobState};
use crate::backup::restore::vm_api;
use crate::operations::runtime::{HandlerFuture, OpContext, Outcome};
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};
use kube::api::{Api, DeleteParams, ListParams, LogParams, PostParams};
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use serde_json::json;
use std::time::Duration;

pub fn run_import_op(ctx: OpContext) -> HandlerFuture {
    Box::pin(async move {
        match import_op(&ctx).await {
            Ok(o) | Err(o) => o,
        }
    })
}

enum Followed {
    Done(H2kvmProgress),
    Failed(String),
    Cancelled,
}

fn deadline_secs() -> u64 {
    std::env::var("ZORVIA_IMPORT_JOB_DEADLINE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12 * 3600)
}

async fn import_op(ctx: &OpContext) -> Result<Outcome, Outcome> {
    let p: ImportOpParams = serde_json::from_value(ctx.op.params.clone())
        .map_err(|e| Outcome::Failed(format!("invalid operation params: {e}")))?;
    let image = h2kvm_image().ok_or_else(|| {
        Outcome::Failed(
            "ZORVIA_H2KVM_IMAGE is not set: point it at your h2kvm CLI image (h2kvm is licensed \
             separately and Zorvia does not ship it)"
                .into(),
        )
    })?;
    let client = ctx.client.client();
    let ns = p.namespace.as_str();
    let vms = vm_api(&client, ns);

    // Everything with this name after a failure was created by this import,
    // which is what makes rollback safe.
    if vms.get(&p.target_vm_name).await.is_ok() {
        return Err(Outcome::Failed(format!(
            "a VM named '{}' already exists in namespace '{ns}'",
            p.target_vm_name
        )));
    }

    let short: String = ctx
        .op
        .id
        .trim_start_matches("op-")
        .chars()
        .take(10)
        .collect();
    let job_name = format!("zorvia-import-{short}");
    ctx.progress("starting h2kvm", 2);
    let job =
        build_import_job(&job_name, &image, &p).map_err(|e| Outcome::Failed(e.to_string()))?;
    let jobs: Api<Job> = Api::namespaced(client.clone(), ns);
    if let Err(e) = jobs.create(&PostParams::default(), &job).await {
        return Err(Outcome::Failed(format!(
            "cannot create the import Job: {e}"
        )));
    }

    match follow(ctx, &client, &p, &job_name).await {
        Followed::Done(prog) => {
            // Trust the cluster, not the log: the VM must actually exist.
            if vms.get(&p.target_vm_name).await.is_err() {
                rollback(&client, &p, &job_name).await;
                return Ok(Outcome::Failed(
                    "h2kvm finished but the VirtualMachine was not created".into(),
                ));
            }
            let mut result = json!({
                "vm_name": p.target_vm_name,
                "namespace": p.namespace,
                "source_vm": p.vm.source_vm,
                "pvc": pvc_name(&p.target_vm_name),
                "stages_completed": prog.stages_done,
            });
            if p.vm.start {
                ctx.progress("verifying boot", 96);
                match wait_for_boot(ctx, ns, &p.target_vm_name).await {
                    Ok(v) => result["validation"] = v,
                    Err(msg) => {
                        // Leave the VM in place for inspection; the import
                        // itself worked but the cutover is not validated.
                        return Ok(Outcome::Failed(format!(
                            "VM '{}' was created but did not boot: {msg} (it was left in place)",
                            p.target_vm_name
                        )));
                    }
                }
            }
            let _ = jobs.delete(&job_name, &DeleteParams::background()).await;
            Ok(Outcome::Succeeded(Some(result)))
        }
        Followed::Failed(msg) => {
            rollback(&client, &p, &job_name).await;
            Ok(Outcome::Failed(msg))
        }
        Followed::Cancelled => {
            rollback(&client, &p, &job_name).await;
            Ok(Outcome::Cancelled)
        }
    }
}

async fn follow(
    ctx: &OpContext,
    client: &kube::Client,
    p: &ImportOpParams,
    job_name: &str,
) -> Followed {
    let ns = p.namespace.as_str();
    let jobs: Api<Job> = Api::namespaced(client.clone(), ns);
    let pods: Api<Pod> = Api::namespaced(client.clone(), ns);
    let started = std::time::Instant::now();
    let give_up = Duration::from_secs(deadline_secs() + 300);
    loop {
        if ctx.cancelled() {
            return Followed::Cancelled;
        }
        let pod_list = pods
            .list(&ListParams::default().labels(&format!("job-name={job_name}")))
            .await
            .map(|l| l.items)
            .unwrap_or_default();
        if let Some(msg) = pod_list.iter().find_map(pod_config_error) {
            return Followed::Failed(msg);
        }
        let pod_name = pod_list.first().and_then(|p| p.metadata.name.clone());
        let mut logs = String::new();
        if let Some(name) = &pod_name {
            let lp = LogParams {
                container: Some("h2kvm".into()),
                tail_lines: Some(400),
                ..Default::default()
            };
            logs = pods.logs(name, &lp).await.unwrap_or_default();
        }
        let prog = parse_h2kvm_logs(&logs);
        ctx.progress(
            prog.current_stage.as_deref().unwrap_or("starting"),
            prog.percent().max(2),
        );

        match jobs.get(job_name).await.map(|j| job_state(&j)) {
            Ok(JobState::Succeeded) => return Followed::Done(prog),
            Ok(JobState::Failed(why)) => {
                let mut msg = prog.failure.clone().unwrap_or(why);
                if prog.stages_started == 0 {
                    // Never got to h2kvm: the nbd loader init container is the
                    // usual culprit, so include what it said.
                    if let Some(name) = &pod_name {
                        let lp = LogParams {
                            container: Some("nbd-loader".into()),
                            tail_lines: Some(20),
                            ..Default::default()
                        };
                        if let Ok(l) = pods.logs(name, &lp).await {
                            let l = l.trim();
                            if !l.is_empty() {
                                msg = format!(
                                    "{msg}; nbd-loader: {}",
                                    l.chars().take(300).collect::<String>()
                                );
                            }
                        }
                    }
                }
                return Followed::Failed(msg);
            }
            Ok(JobState::Running) | Err(_) => {}
        }
        if started.elapsed() >= give_up {
            return Followed::Failed("timed out waiting for the import Job".into());
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

async fn wait_for_boot(ctx: &OpContext, ns: &str, vm: &str) -> Result<serde_json::Value, String> {
    let timeout = Duration::from_secs(
        std::env::var("ZORVIA_IMPORT_BOOT_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(600),
    );
    let t0 = std::time::Instant::now();
    let mut last = String::from("no VirtualMachineInstance");
    loop {
        if let Ok(r) = ctx.client.guest_ready_report(ns, vm).await {
            last = r.reason.clone();
            if r.running {
                // Guests migrated from VMware often have no qemu-guest-agent,
                // so it is reported, not required.
                return Ok(json!({
                    "running": true,
                    "guest_agent": r.agent_connected,
                    "boot_seconds": t0.elapsed().as_secs(),
                }));
            }
        }
        if t0.elapsed() >= timeout {
            return Err(format!("not Running after {}s ({last})", timeout.as_secs()));
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Remove what this import may have created: the Job, the VM, its root PVC and
/// the upload DataVolume of the same name. Best effort.
async fn rollback(client: &kube::Client, p: &ImportOpParams, job_name: &str) {
    let ns = p.namespace.as_str();
    let jobs: Api<Job> = Api::namespaced(client.clone(), ns);
    let _ = jobs.delete(job_name, &DeleteParams::background()).await;
    let _ = vm_api(client, ns)
        .delete(&p.target_vm_name, &DeleteParams::default())
        .await;
    let dv_ar = ApiResource::from_gvk(&GroupVersionKind::gvk(
        "cdi.kubevirt.io",
        "v1beta1",
        "DataVolume",
    ));
    let dvs: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &dv_ar);
    let name = pvc_name(&p.target_vm_name);
    let _ = dvs.delete(&name, &DeleteParams::default()).await;
    let pvcs: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), ns);
    let _ = pvcs.delete(&name, &DeleteParams::default()).await;
}
