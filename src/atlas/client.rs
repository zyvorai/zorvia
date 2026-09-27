// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use super::models::*;
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub token: Option<String>,
    pub default_tenant_id: Option<String>,
    pub timeout: Duration,
    pub allow_invalid_tls: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let base_url = match std::env::var("ATLAS_URL") {
            Ok(value) if !value.trim().is_empty() => value.trim().trim_end_matches('/').to_string(),
            _ => return Ok(None),
        };
        let parsed = reqwest::Url::parse(&base_url)
            .map_err(|e| anyhow::anyhow!("invalid ATLAS_URL: {e}"))?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            anyhow::bail!("ATLAS_URL must use http:// or https://");
        }
        let timeout_secs = std::env::var("ATLAS_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0 && *v <= 300)
            .unwrap_or(30);
        let allow_invalid_tls = std::env::var("ATLAS_TLS_INSECURE")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        Ok(Some(Self {
            base_url,
            token: std::env::var("ATLAS_TOKEN").ok().filter(|v| !v.is_empty()),
            default_tenant_id: std::env::var("ATLAS_TENANT_ID")
                .ok()
                .filter(|v| !v.is_empty()),
            timeout: Duration::from_secs(timeout_secs),
            allow_invalid_tls,
        }))
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Atlas request failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("Atlas returned HTTP {status}: {message}")]
    Upstream {
        status: u16,
        code: Option<String>,
        message: String,
    },
    #[error("Atlas tenant is required; set ATLAS_TENANT_ID or pass a tenant_id")]
    MissingTenant,
}

#[derive(Debug, serde::Deserialize)]
struct UpstreamEnvelope {
    error: Option<UpstreamError>,
}

#[derive(Debug, serde::Deserialize)]
struct UpstreamError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Clone)]
pub struct Client {
    config: Config,
    http: reqwest::Client,
}

impl Client {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        Config::from_env()?.map(Self::new).transpose()
    }

    pub fn new(config: Config) -> anyhow::Result<Self> {
        if config.allow_invalid_tls {
            log::warn!(
                "ATLAS_TLS_INSECURE is enabled; Atlas TLS certificates will not be verified"
            );
        }
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .danger_accept_invalid_certs(config.allow_invalid_tls)
            .user_agent(format!(
                "zorvia/{}/atlas-adapter",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;
        Ok(Self { config, http })
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    pub fn configured_tenant_id(&self) -> Option<&str> {
        self.config.default_tenant_id.as_deref()
    }

    fn tenant_id<'a>(&'a self, tenant_id: Option<&'a str>) -> Result<&'a str, Error> {
        tenant_id
            .filter(|v| !v.trim().is_empty())
            .or(self.config.default_tenant_id.as_deref())
            .ok_or(Error::MissingTenant)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.config.base_url, path.trim_start_matches('/'))
    }

    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let mut req = self.http.request(method, self.endpoint(path));
        if let Some(token) = &self.config.token {
            req = req.bearer_auth(token);
        }
        req
    }

    async fn decode<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, Error> {
        let response = request.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.json::<T>().await?);
        }
        Err(Self::decode_error(
            status,
            response.text().await.unwrap_or_default(),
        ))
    }

    fn decode_error(status: StatusCode, body: String) -> Error {
        let parsed = serde_json::from_str::<UpstreamEnvelope>(&body)
            .ok()
            .and_then(|v| v.error);
        let message = parsed
            .as_ref()
            .and_then(|e| e.message.clone())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                let trimmed = body.trim();
                if trimmed.is_empty() {
                    status
                        .canonical_reason()
                        .unwrap_or("upstream error")
                        .to_string()
                } else {
                    trimmed.chars().take(512).collect()
                }
            });
        Error::Upstream {
            status: status.as_u16(),
            code: parsed.and_then(|e| e.code),
            message,
        }
    }

    // ── Unauthenticated (root, not under /api/atlas/v1) ──

    pub async fn health(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/health")).await
    }

    pub async fn version(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/version")).await
    }

    // ── Bearer-authenticated, /api/atlas/v1/* ──

    pub async fn list_backends(&self) -> Result<Vec<StorageBackend>, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/backends"))
            .await
    }

    pub async fn backends_summary(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/backends/summary"))
            .await
    }

    /// Registers a backend row -- synchronous, not an async job (unlike the
    /// volume writes). Only `nfs`/`zfs` `backend_type`s get instantiated
    /// live; others land as a `pending` catalog row.
    pub async fn create_backend(
        &self,
        request: CreateBackendRequest,
    ) -> Result<StorageBackend, Error> {
        self.decode(
            self.request(Method::POST, "/api/atlas/v1/backends")
                .json(&request),
        )
        .await
    }

    /// Refused (`400`) while volumes still reference the backend unless
    /// `purge` is set, which first drops the backend's orphaned inventory
    /// rows (no real storage is touched by purge itself).
    pub async fn delete_backend(&self, id: &str, purge: bool) -> Result<serde_json::Value, Error> {
        let path = format!("/api/atlas/v1/backends/{}", urlencoding::encode(id));
        let mut req = self.request(Method::DELETE, &path);
        if purge {
            req = req.query(&[("purge", "true")]);
        }
        self.decode(req).await
    }

    pub async fn discover_backend(&self, id: &str) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::POST,
            &format!(
                "/api/atlas/v1/backends/{}/discover",
                urlencoding::encode(id)
            ),
        ))
        .await
    }

    pub async fn cordon_backend(&self, id: &str) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::POST,
            &format!("/api/atlas/v1/backends/{}/cordon", urlencoding::encode(id)),
        ))
        .await
    }

    pub async fn uncordon_backend(&self, id: &str) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::POST,
            &format!(
                "/api/atlas/v1/backends/{}/uncordon",
                urlencoding::encode(id)
            ),
        ))
        .await
    }

    /// Whether Atlas's job engine is paused. Paused jobs stay `queued`
    /// until resumed.
    pub async fn get_maintenance(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/maintenance"))
            .await
    }

    pub async fn set_maintenance(&self, paused: bool) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(Method::POST, "/api/atlas/v1/maintenance")
                .json(&serde_json::json!({ "paused": paused })),
        )
        .await
    }

    pub async fn list_orphans(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/maintenance/orphans"))
            .await
    }

    /// Report-only (always `200`); `ready`/`blockers` in the response is the
    /// actual gate -- aggregates cluster health, open critical alerts,
    /// in-flight jobs, and CDC lag.
    pub async fn upgrade_preflight(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/upgrade/preflight"))
            .await
    }

    pub async fn list_osds(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/osds"))
            .await
    }

    /// Drain an OSD (`ceph osd out`) -- async job, admin-role.
    pub async fn osd_out(&self, osd_id: i64) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::POST, &format!("/api/atlas/v1/osds/{osd_id}/out")))
            .await
    }

    /// Return a drained OSD to service (`ceph osd in`) -- async job, admin-role.
    pub async fn osd_in(&self, osd_id: i64) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::POST, &format!("/api/atlas/v1/osds/{osd_id}/in")))
            .await
    }

    /// Reweight an OSD (`ceph osd reweight`, `weight` in `[0.0, 1.0]`) --
    /// async job, admin-role.
    pub async fn osd_reweight(&self, osd_id: i64, weight: f64) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(
                Method::POST,
                &format!("/api/atlas/v1/osds/{osd_id}/reweight"),
            )
            .query(&[("weight", weight.to_string())]),
        )
        .await
    }

    pub async fn list_clusters(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/clusters"))
            .await
    }

    pub async fn cluster_health(&self, id: &str) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::GET,
            &format!("/api/atlas/v1/clusters/{}/health", urlencoding::encode(id)),
        ))
        .await
    }

    pub async fn list_pools(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/pools"))
            .await
    }

    pub async fn ceph_status(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/ceph/status"))
            .await
    }

    pub async fn ceph_df(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/ceph/df"))
            .await
    }

    pub async fn list_storage_classes(&self) -> Result<Vec<StorageClassInfo>, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/storage-classes"))
            .await
    }

    pub async fn list_volumes(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/volumes"))
            .await
    }

    pub async fn create_volume(
        &self,
        mut request: CreateVolumeRequest,
    ) -> Result<serde_json::Value, Error> {
        if request.tenant_id.trim().is_empty() {
            request.tenant_id = self.tenant_id(None)?.to_string();
        }
        self.decode(
            self.request(Method::POST, "/api/atlas/v1/volumes")
                .json(&request),
        )
        .await
    }

    /// Grows a volume. Atlas rejects `new_size_bytes <= current size` itself
    /// (`AppError::Validation`), so this doesn't duplicate that check.
    pub async fn expand_volume(
        &self,
        id: &str,
        new_size_bytes: i64,
    ) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(
                Method::POST,
                &format!("/api/atlas/v1/volumes/{}/expand", urlencoding::encode(id)),
            )
            .json(&serde_json::json!({ "new_size_bytes": new_size_bytes })),
        )
        .await
    }

    /// Deletes a volume. `confirm` must be `true` for production/protected-class
    /// volumes (Atlas's own `volume_requires_delete_confirm` check) -- Atlas
    /// returns a `400 Validation` error naming that requirement if it's needed
    /// and missing, rather than Zorvia silently guessing when to set it.
    pub async fn delete_volume(&self, id: &str, confirm: bool) -> Result<serde_json::Value, Error> {
        let path = format!("/api/atlas/v1/volumes/{}", urlencoding::encode(id));
        let mut req = self.request(Method::DELETE, &path);
        if confirm {
            req = req.query(&[("confirm", "true")]);
        }
        self.decode(req).await
    }

    /// Every Atlas write (including volume create/expand/delete) returns a
    /// job id -- this is how to find out what actually happened to it.
    /// Atlas caps this list at 100 most-recent jobs server-side; there's no
    /// pagination to request more.
    pub async fn list_jobs(&self) -> Result<Vec<JobRecord>, Error> {
        self.decode(self.request(Method::GET, "/api/atlas/v1/jobs"))
            .await
    }

    pub async fn get_job(&self, id: &str) -> Result<JobRecord, Error> {
        self.decode(self.request(
            Method::GET,
            &format!("/api/atlas/v1/jobs/{}", urlencoding::encode(id)),
        ))
        .await
    }

    /// Atlas requires admin-role on the token for this; returns `409` if the
    /// job is already terminal (succeeded/failed) -- surfaced to the caller
    /// as a normal `Error::Upstream`, not treated as a client-level error.
    pub async fn cancel_job(&self, id: &str) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::POST,
            &format!("/api/atlas/v1/jobs/{}/cancel", urlencoding::encode(id)),
        ))
        .await
    }

    // ── RBD images -- a separate identity space (rbd:<pool>/<image>) from
    // the StorageVolume abstraction the volume routes above use. All writes
    // here are async jobs (Atlas's `accepted()` envelope), same shape as
    // the volume writes, except `refresh_rbd_usage` which is synchronous. ──

    fn rbd_path(pool: &str, image: &str) -> String {
        format!(
            "/api/atlas/v1/rbd-images/{}/{}",
            urlencoding::encode(pool),
            urlencoding::encode(image)
        )
    }

    pub async fn list_rbd_images(&self, pool: Option<&str>) -> Result<serde_json::Value, Error> {
        let mut req = self.request(Method::GET, "/api/atlas/v1/rbd-images");
        if let Some(pool) = pool {
            req = req.query(&[("pool", pool)]);
        }
        self.decode(req).await
    }

    pub async fn create_rbd_image(
        &self,
        request: CreateRbdImageRequest,
    ) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(Method::POST, "/api/atlas/v1/rbd-images")
                .json(&request),
        )
        .await
    }

    pub async fn delete_rbd_image(
        &self,
        pool: &str,
        image: &str,
    ) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::DELETE, &Self::rbd_path(pool, image)))
            .await
    }

    /// Snapshot+protect the parent and create a COW clone.
    pub async fn clone_rbd_image(
        &self,
        pool: &str,
        image: &str,
        request: CloneRbdImageRequest,
    ) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(
                Method::POST,
                &format!("{}/clone", Self::rbd_path(pool, image)),
            )
            .json(&request),
        )
        .await
    }

    /// Grows by default; `allow_shrink: true` permits a shrink (Atlas warns
    /// this can lose data past the new size).
    pub async fn resize_rbd_image(
        &self,
        pool: &str,
        image: &str,
        size_bytes: i64,
        allow_shrink: bool,
    ) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(
                Method::POST,
                &format!("{}/resize", Self::rbd_path(pool, image)),
            )
            .json(&serde_json::json!({ "size_bytes": size_bytes, "allow_shrink": allow_shrink })),
        )
        .await
    }

    /// Live-migrates to another pool (`rbd migration prepare -> execute -> commit`).
    pub async fn migrate_rbd_image(
        &self,
        pool: &str,
        image: &str,
        dest_pool: &str,
    ) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(
                Method::POST,
                &format!("{}/migrate", Self::rbd_path(pool, image)),
            )
            .query(&[("dest_pool", dest_pool)]),
        )
        .await
    }

    pub async fn flatten_rbd_image(
        &self,
        pool: &str,
        image: &str,
    ) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::POST,
            &format!("{}/flatten", Self::rbd_path(pool, image)),
        ))
        .await
    }

    /// Caps IOPS/bandwidth; `0` clears a cap. At least one of `iops`/`bps`
    /// is required -- Atlas rejects the request with `400` otherwise.
    pub async fn qos_rbd_image(
        &self,
        pool: &str,
        image: &str,
        iops: Option<i64>,
        bps: Option<i64>,
    ) -> Result<serde_json::Value, Error> {
        let mut query = Vec::new();
        if let Some(iops) = iops {
            query.push(("iops".to_string(), iops.to_string()));
        }
        if let Some(bps) = bps {
            query.push(("bps".to_string(), bps.to_string()));
        }
        self.decode(
            self.request(
                Method::POST,
                &format!("{}/qos", Self::rbd_path(pool, image)),
            )
            .query(&query),
        )
        .await
    }

    pub async fn list_rbd_snapshots(
        &self,
        pool: &str,
        image: &str,
    ) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::GET,
            &format!("{}/snapshots", Self::rbd_path(pool, image)),
        ))
        .await
    }

    pub async fn create_rbd_snapshot(
        &self,
        pool: &str,
        image: &str,
        name: &str,
    ) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(
                Method::POST,
                &format!("{}/snapshots", Self::rbd_path(pool, image)),
            )
            .json(&serde_json::json!({ "name": name })),
        )
        .await
    }

    /// Rolls the image back to the named snapshot (destructive; admin-role
    /// on Atlas's side).
    pub async fn rollback_rbd_image(
        &self,
        pool: &str,
        image: &str,
        snapshot_name: &str,
    ) -> Result<serde_json::Value, Error> {
        self.decode(
            self.request(
                Method::POST,
                &format!("{}/rollback", Self::rbd_path(pool, image)),
            )
            .json(&serde_json::json!({ "name": snapshot_name })),
        )
        .await
    }

    pub async fn delete_rbd_snapshot(
        &self,
        pool: &str,
        image: &str,
        snapshot_name: &str,
    ) -> Result<serde_json::Value, Error> {
        self.decode(self.request(
            Method::DELETE,
            &format!(
                "{}/snapshots/{}",
                Self::rbd_path(pool, image),
                urlencoding::encode(snapshot_name)
            ),
        ))
        .await
    }

    /// Recomputes `used_bytes` for every RBD-backed volume (`rbd du`) --
    /// synchronous, not an async job, unlike every other write in this
    /// section.
    pub async fn refresh_rbd_usage(&self) -> Result<serde_json::Value, Error> {
        self.decode(self.request(Method::POST, "/api/atlas/v1/rbd-usage/refresh"))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_normalizes_slashes() {
        let client = Client::new(Config {
            base_url: "http://127.0.0.1:9090".into(),
            token: None,
            default_tenant_id: Some("global".into()),
            timeout: Duration::from_secs(1),
            allow_invalid_tls: false,
        })
        .unwrap();
        assert_eq!(
            client.endpoint("/api/atlas/v1/backends"),
            "http://127.0.0.1:9090/api/atlas/v1/backends"
        );
    }

    #[test]
    fn atlas_error_envelope_is_preserved() {
        let err = Client::decode_error(
            StatusCode::CONFLICT,
            r#"{"error":{"code":"CONFLICT","message":"tenant quota exceeded"}}"#.into(),
        );
        match err {
            Error::Upstream {
                status,
                code,
                message,
            } => {
                assert_eq!(status, 409);
                assert_eq!(code.as_deref(), Some("CONFLICT"));
                assert_eq!(message, "tenant quota exceeded");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn missing_tenant_is_rejected_without_a_default() {
        let client = Client::new(Config {
            base_url: "http://127.0.0.1:9090".into(),
            token: None,
            default_tenant_id: None,
            timeout: Duration::from_secs(1),
            allow_invalid_tls: false,
        })
        .unwrap();
        assert!(matches!(client.tenant_id(None), Err(Error::MissingTenant)));
        assert_eq!(client.tenant_id(Some("acme")).unwrap(), "acme");
    }

    // ── Live-instance checks -- not run by default, no Atlas instance in CI.
    // Start one first: `cd ../atlas && cargo run -p atlas-gateway` (dev config,
    // fake Ceph driver, no auth required, listens on 127.0.0.1:5110 by default
    // -- see `../atlas/README.md`'s Quickstart), then:
    //   cargo test --features web -p zorvia atlas::client::tests::live -- --ignored --nocapture

    fn live_client() -> Client {
        Client::new(Config {
            base_url: "http://127.0.0.1:5110".into(),
            token: None,
            default_tenant_id: Some("global".into()),
            timeout: Duration::from_secs(5),
            allow_invalid_tls: false,
        })
        .unwrap()
    }

    #[tokio::test]
    #[ignore]
    async fn live_health_and_version() {
        let c = live_client();
        let health = c.health().await.unwrap();
        assert_eq!(health["status"], "ok");
        c.version().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn live_list_backends_matches_real_contract() {
        let c = live_client();
        let backends = c.list_backends().await.unwrap();
        assert!(!backends.is_empty(), "fake driver seeds one backend");
        assert_eq!(backends[0].backend_type, BackendType::Ceph);
    }

    #[tokio::test]
    #[ignore]
    async fn live_list_clusters_pools_volumes() {
        let c = live_client();
        let clusters = c.list_clusters().await.unwrap();
        assert!(clusters.as_array().is_some_and(|a| !a.is_empty()));
        let pools = c.list_pools().await.unwrap();
        assert!(pools.as_array().is_some_and(|a| !a.is_empty()));
        // Fake driver seeds volumes too; this just confirms the route/shape,
        // not a specific count.
        c.list_volumes().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn live_create_volume_round_trips() {
        let c = live_client();
        let name = format!("zorvia-live-test-{}", chrono::Utc::now().timestamp_millis());
        let result = c
            .create_volume(CreateVolumeRequest {
                tenant_id: String::new(), // filled from default_tenant_id
                name: name.clone(),
                size_bytes: 1024 * 1024 * 1024,
                kind: VolumeKind::Block,
                policy: None,
                pool: None,
                owner: Some(Owner::for_vm("zorvia-live-test-vm")),
                kubernetes: None,
            })
            .await
            .unwrap();
        // create_volume's handler returns (StatusCode, Json<Value>) -- assert
        // it's a real object, not an error envelope masquerading as 200.
        assert!(
            result.get("error").is_none(),
            "unexpected error in response: {result:?}"
        );
    }

    // The fake driver seeds `vol_rbd_nvme_prod_web-01-root` and
    // `vol_rbd_nvme_prod_billing-db-01-root` on every fresh `atlas.db` -- see
    // `../atlas/crates/atlas-discovery`'s fake-mode seed data. Used here so
    // expand/delete have a real volume to act on without depending on
    // create_volume's own job (which fails in this dev setup -- no
    // ATLAS_KUBECONFIG attached, see the module doc above).

    #[tokio::test]
    #[ignore]
    async fn live_expand_volume_round_trips() {
        let c = live_client();
        let result = c
            .expand_volume("vol_rbd_nvme_prod_web-01-root", 50 * 1024 * 1024 * 1024)
            .await
            .unwrap();
        assert!(
            result.get("error").is_none(),
            "unexpected error in response: {result:?}"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn live_delete_volume_requires_confirm_for_protected_class() {
        let c = live_client();
        // Without confirm: Atlas's own protected-class check should reject this
        // with a real, specific error -- not silently succeed.
        let err = c
            .delete_volume("vol_rbd_nvme_prod_billing-db-01-root", false)
            .await
            .unwrap_err();
        match err {
            Error::Upstream {
                status, message, ..
            } => {
                assert_eq!(status, 400);
                assert!(
                    message.contains("confirm=true"),
                    "expected the confirm-required message, got: {message}"
                );
            }
            other => panic!("expected an Upstream 400, got: {other:?}"),
        }

        // With confirm: accepted.
        let result = c
            .delete_volume("vol_rbd_nvme_prod_billing-db-01-root", true)
            .await
            .unwrap();
        assert!(
            result.get("error").is_none(),
            "unexpected error in response: {result:?}"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn live_job_list_get_and_cancel_round_trip() {
        let c = live_client();

        // Create a volume to get a real job id -- exercises list/get end to
        // end against an actual job this instance just created, not a
        // fabricated id.
        let name = format!(
            "zorvia-live-job-test-{}",
            chrono::Utc::now().timestamp_millis()
        );
        let create_result = c
            .create_volume(CreateVolumeRequest {
                tenant_id: String::new(),
                name,
                size_bytes: 1024 * 1024 * 1024,
                kind: VolumeKind::Block,
                policy: None,
                pool: None,
                owner: None,
                kubernetes: None,
            })
            .await
            .unwrap();
        let job_id = create_result["job_id"]
            .as_str()
            .expect("create_volume response has a job_id")
            .to_string();

        let jobs = c.list_jobs().await.unwrap();
        assert!(
            jobs.iter().any(|j| j.id == job_id),
            "expected job {job_id} in list_jobs()"
        );

        let job = c.get_job(&job_id).await.unwrap();
        assert_eq!(job.id, job_id);
        assert_eq!(job.job_type, "volume.create");

        // This local instance has no Kubernetes cluster attached (see the
        // create_volume test above), so the job fails fast rather than
        // staying queued/running -- cancel on an already-terminal job
        // should get Atlas's real 409, not silently succeed.
        if job.is_terminal() {
            let err = c.cancel_job(&job_id).await.unwrap_err();
            match err {
                Error::Upstream { status, .. } => assert_eq!(status, 409),
                other => panic!("expected an Upstream 409, got: {other:?}"),
            }
        } else {
            let result = c.cancel_job(&job_id).await.unwrap();
            assert_eq!(result["cancelled"], true);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn live_maintenance_reads_and_preflight() {
        let c = live_client();
        let maint = c.get_maintenance().await.unwrap();
        assert!(maint.get("paused").is_some());
        c.list_orphans().await.unwrap();
        let preflight = c.upgrade_preflight().await.unwrap();
        // Report-only: always 200, `ready` is the real gate -- just confirm
        // the shape, not a particular verdict.
        assert!(preflight.get("ready").is_some() || preflight.get("blockers").is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn live_set_maintenance_round_trips() {
        let c = live_client();
        let paused = c.set_maintenance(true).await.unwrap();
        assert_eq!(paused["paused"], true);
        // Always resume -- leaving the fake driver's job engine paused would
        // silently wedge every other live test run against this instance.
        let resumed = c.set_maintenance(false).await.unwrap();
        assert_eq!(resumed["paused"], false);
    }

    #[tokio::test]
    #[ignore]
    async fn live_backend_create_cordon_uncordon_delete_round_trip() {
        let c = live_client();
        let backend = c
            .create_backend(CreateBackendRequest {
                name: format!(
                    "zorvia-live-test-backend-{}",
                    chrono::Utc::now().timestamp_millis()
                ),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(backend.status, "pending"); // no backend_type given -> defaults to Ceph -> "pending" row, no live driver
        assert!(!backend.cordoned);

        let cordoned = c.cordon_backend(&backend.id).await.unwrap();
        assert_eq!(cordoned["cordoned"], true);
        let uncordoned = c.uncordon_backend(&backend.id).await.unwrap();
        assert_eq!(uncordoned["cordoned"], false);

        let deleted = c.delete_backend(&backend.id, false).await.unwrap();
        assert_eq!(deleted["deleted"], true);
    }

    #[tokio::test]
    #[ignore]
    async fn live_osd_list_and_out_in_round_trip() {
        let c = live_client();
        let osds = c.list_osds().await.unwrap();
        let osd_id = osds
            .as_array()
            .and_then(|a| a.first())
            .and_then(|o| o.get("id"))
            .and_then(|v| v.as_i64())
            .expect("fake driver seeds at least one OSD");

        let out_result = c.osd_out(osd_id).await.unwrap();
        assert!(out_result.get("error").is_none());
        let in_result = c.osd_in(osd_id).await.unwrap();
        assert!(in_result.get("error").is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn live_rbd_image_full_lifecycle() {
        let c = live_client();
        let pool = "rbd-nvme-prod"; // seeded by the fake driver, see live_list_backends... fixtures
        let name = format!("zorvia-live-rbd-{}", chrono::Utc::now().timestamp_millis());

        let created = c
            .create_rbd_image(CreateRbdImageRequest {
                name: name.clone(),
                size_bytes: 1024 * 1024 * 1024,
                pool: Some(pool.to_string()),
                tenant_id: None,
            })
            .await
            .unwrap();
        assert!(created.get("error").is_none(), "create failed: {created:?}");

        let resized = c
            .resize_rbd_image(pool, &name, 2 * 1024 * 1024 * 1024, false)
            .await
            .unwrap();
        assert!(resized.get("error").is_none());

        let qos = c.qos_rbd_image(pool, &name, Some(500), None).await.unwrap();
        assert!(qos.get("error").is_none());

        let snap_name = "zorvia-live-snap";
        let snap = c.create_rbd_snapshot(pool, &name, snap_name).await.unwrap();
        assert!(snap.get("error").is_none());

        let snaps = c.list_rbd_snapshots(pool, &name).await.unwrap();
        assert!(snaps.get("error").is_none());

        let rolled_back = c.rollback_rbd_image(pool, &name, snap_name).await.unwrap();
        assert!(rolled_back.get("error").is_none());

        let images = c.list_rbd_images(Some(pool)).await.unwrap();
        assert!(images.get("error").is_none());

        let usage = c.refresh_rbd_usage().await.unwrap();
        assert!(usage.get("updated").is_some());

        // Clean up: snapshot first, then the image, same order Atlas itself
        // requires (a protected snapshot backing a clone would refuse
        // image delete, though this test never cloned).
        let snap_deleted = c.delete_rbd_snapshot(pool, &name, snap_name).await.unwrap();
        assert!(snap_deleted.get("error").is_none());
        let deleted = c.delete_rbd_image(pool, &name).await.unwrap();
        assert!(deleted.get("error").is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn live_rbd_clone_and_flatten() {
        let c = live_client();
        let pool = "rbd-nvme-prod";
        let parent = format!(
            "zorvia-live-rbd-parent-{}",
            chrono::Utc::now().timestamp_millis()
        );
        let clone_name = format!("{parent}-clone");

        c.create_rbd_image(CreateRbdImageRequest {
            name: parent.clone(),
            size_bytes: 1024 * 1024 * 1024,
            pool: Some(pool.to_string()),
            tenant_id: None,
        })
        .await
        .unwrap();

        let cloned = c
            .clone_rbd_image(
                pool,
                &parent,
                CloneRbdImageRequest {
                    name: clone_name.clone(),
                    snap: None,
                    tenant_id: None,
                },
            )
            .await
            .unwrap();
        assert!(cloned.get("error").is_none(), "clone failed: {cloned:?}");

        let flattened = c.flatten_rbd_image(pool, &clone_name).await.unwrap();
        assert!(flattened.get("error").is_none());

        // Clean up both images (clone first -- it may still reference the
        // parent's auto-created `<clone>-base` snapshot).
        c.delete_rbd_image(pool, &clone_name).await.unwrap();
        c.delete_rbd_image(pool, &parent).await.unwrap();
    }
}
