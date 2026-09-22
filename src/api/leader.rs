//! Kubernetes Lease-based leader election for background schedulers.
//!
//! Enabled when `ZORVIA_LEADER_ELECTION=1`. Without it (or if the Lease API
//! is unreachable), this process always considers itself leader so single-
//! replica lab deploys keep working.

use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::MicroTime;
use kube::api::{Api, Patch, PatchParams, PostParams};
use kube::Client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static IS_LEADER: AtomicBool = AtomicBool::new(true);

pub fn is_leader() -> bool {
    IS_LEADER.load(Ordering::Relaxed)
}

fn env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        }
        Err(_) => false,
    }
}

/// Spawn a background renew loop when leader election is enabled.
pub fn spawn_leader_election() {
    if !env_truthy("ZORVIA_LEADER_ELECTION") {
        log::info!("Leader election disabled; this process runs schedulers");
        IS_LEADER.store(true, Ordering::Relaxed);
        return;
    }
    IS_LEADER.store(false, Ordering::Relaxed);
    tokio::spawn(async move {
        if let Err(e) = run_election_loop().await {
            log::error!("Leader election failed ({e}); assuming leadership for schedulers");
            IS_LEADER.store(true, Ordering::Relaxed);
        }
    });
}

async fn run_election_loop() -> anyhow::Result<()> {
    let client = Client::try_default().await?;
    let ns = std::env::var("ZORVIA_LEADER_NAMESPACE")
        .or_else(|_| std::env::var("POD_NAMESPACE"))
        .unwrap_or_else(|_| "zorvia-system".into());
    let lease_name =
        std::env::var("ZORVIA_LEADER_LEASE").unwrap_or_else(|_| "zorvia-schedulers".into());
    let identity =
        std::env::var("POD_NAME").unwrap_or_else(|_| format!("zorvia-{}", std::process::id()));
    let api: Api<Lease> = Api::namespaced(client, &ns);
    let lease_duration = Duration::from_secs(15);
    let renew_period = Duration::from_secs(5);

    log::info!(
        "Leader election: lease={}/{} identity={}",
        ns,
        lease_name,
        identity
    );

    loop {
        match try_acquire_or_renew(&api, &lease_name, &identity, lease_duration).await {
            Ok(true) => {
                if !IS_LEADER.swap(true, Ordering::Relaxed) {
                    log::info!("Acquired scheduler leadership ({identity})");
                }
            }
            Ok(false) => {
                if IS_LEADER.swap(false, Ordering::Relaxed) {
                    log::info!("Lost scheduler leadership ({identity})");
                }
            }
            Err(e) => {
                log::warn!("Lease renew error: {e}");
                IS_LEADER.store(false, Ordering::Relaxed);
            }
        }
        tokio::time::sleep(renew_period).await;
    }
}

fn format_k8s_micro_time(ts: k8s_openapi::jiff::Timestamp) -> String {
    // Kubernetes MicroTime parsing expects RFC3339 with *exactly* 6 fractional
    // digits (`.000000`). jiff's Display omits trailing zeros, which the
    // apiserver rejects.
    let s = ts.to_string();
    let body = s.trim_end_matches('Z');
    if let Some((date, frac)) = body.split_once('.') {
        let mut digits: String = frac
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(6)
            .collect();
        while digits.len() < 6 {
            digits.push('0');
        }
        format!("{date}.{digits}Z")
    } else {
        format!("{body}.000000Z")
    }
}

fn micro_now() -> k8s_openapi::jiff::Timestamp {
    let t = k8s_openapi::jiff::Timestamp::now();
    k8s_openapi::jiff::Timestamp::from_microsecond(t.as_microsecond()).unwrap_or(t)
}

async fn try_acquire_or_renew(
    api: &Api<Lease>,
    name: &str,
    identity: &str,
    lease_duration: Duration,
) -> anyhow::Result<bool> {
    let now = micro_now();
    match api.get(name).await {
        Ok(existing) => {
            let holder = existing
                .spec
                .as_ref()
                .and_then(|s| s.holder_identity.as_deref())
                .unwrap_or("");
            let renew_time = existing
                .spec
                .as_ref()
                .and_then(|s| s.renew_time.as_ref())
                .map(|t| t.0);
            let expired = renew_time
                .map(|t| now.as_second() > t.as_second() + lease_duration.as_secs() as i64)
                .unwrap_or(true);

            if holder == identity || expired || holder.is_empty() {
                let resource_version = existing.metadata.resource_version.clone();
                let renew = format_k8s_micro_time(now);
                let acquire = existing
                    .spec
                    .as_ref()
                    .and_then(|s| s.acquire_time.as_ref())
                    .map(|t| format_k8s_micro_time(t.0))
                    .unwrap_or_else(|| renew.clone());
                let patch = serde_json::json!({
                    "metadata": { "resourceVersion": resource_version },
                    "spec": {
                        "holderIdentity": identity,
                        "leaseDurationSeconds": lease_duration.as_secs() as i32,
                        "renewTime": renew,
                        "acquireTime": acquire,
                    }
                });
                api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
                    .await?;
                return Ok(true);
            }
            Ok(false)
        }
        Err(kube::Error::Api(ae)) if ae.code == 404 => {
            let lease = Lease {
                metadata: kube::api::ObjectMeta {
                    name: Some(name.to_string()),
                    ..Default::default()
                },
                spec: Some(k8s_openapi::api::coordination::v1::LeaseSpec {
                    holder_identity: Some(identity.to_string()),
                    lease_duration_seconds: Some(lease_duration.as_secs() as i32),
                    acquire_time: Some(MicroTime(now)),
                    renew_time: Some(MicroTime(now)),
                    ..Default::default()
                }),
            };
            api.create(&PostParams::default(), &lease).await?;
            Ok(true)
        }
        Err(e) => Err(e.into()),
    }
}

/// Helper used by scheduler loops: skip a tick when not leader.
pub fn skip_if_follower(task: &str) -> bool {
    if is_leader() {
        return false;
    }
    log::trace!("Skipping {task}: not lease leader");
    true
}
