// HTTP Server - Real web server using axum for serving API and dashboard
//
// This module is only compiled when the "web" feature is enabled.

#[cfg(feature = "web")]
pub mod web {
    use crate::api::{ApiResponse, HttpMethod, RequestContext};
    use crate::kube::KubeClient;
    use crate::tui::state::VmInfo;
    use axum::{
        extract::{DefaultBodyLimit, Path, Query, State},
        http::{header, HeaderMap, StatusCode},
        middleware,
        response::{IntoResponse, Json},
        routing::{delete, get, post, put},
        Router,
    };
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::sync::RwLock;
    use tower_http::cors::{AllowOrigin, CorsLayer};
    use tower_http::services::{ServeDir, ServeFile};
    use tower_http::timeout::TimeoutLayer;

    #[path = "fabric_vm_handlers.rs"]
    mod fabric_vm_handlers;
    use fabric_vm_handlers::*;

    #[path = "kryton_handlers.rs"]
    mod kryton_handlers;
    use kryton_handlers::*;

    #[path = "ws_proxy_handlers.rs"]
    mod ws_proxy_handlers;
    use ws_proxy_handlers::{ws_console, ws_ssh, ws_vnc};

    #[path = "hotplug_handlers.rs"]
    mod hotplug_handlers;
    use hotplug_handlers::*;

    #[path = "migration_handlers.rs"]
    mod migration_handlers;
    use migration_handlers::*;

    #[path = "rook_handlers.rs"]
    mod rook_handlers;
    use rook_handlers::*;

    #[path = "disk_network_handlers.rs"]
    mod disk_network_handlers;
    use disk_network_handlers::*;

    #[path = "drift_handlers.rs"]
    mod drift_handlers;
    use drift_handlers::*;

    /// Simple sliding-window rate limiter state.
    struct RateLimiterState {
        /// Number of requests in the current window
        count: AtomicU64,
        /// Start of the current window (unix timestamp seconds)
        window_start: AtomicU64,
        /// Maximum requests per window
        max_requests: u64,
        /// Window duration in seconds
        window_secs: u64,
    }

    impl RateLimiterState {
        fn new(max_requests: u64, window_secs: u64) -> Self {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Self {
                count: AtomicU64::new(0),
                window_start: AtomicU64::new(now),
                max_requests,
                window_secs,
            }
        }

        fn check_rate_limit(&self) -> bool {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let window_start = self.window_start.load(Ordering::Relaxed);

            // Reset window if expired
            if now - window_start >= self.window_secs {
                self.window_start.store(now, Ordering::Relaxed);
                self.count.store(1, Ordering::Relaxed);
                return true;
            }

            let count = self.count.fetch_add(1, Ordering::Relaxed);
            count < self.max_requests
        }
    }

    /// TLS configuration for the API server.
    #[derive(Clone, Debug)]
    pub struct TlsConfig {
        pub cert_path: String,
        pub key_path: String,
    }

    impl TlsConfig {
        /// Validate that the cert and key files exist on disk.
        pub fn validate(&self) -> anyhow::Result<()> {
            if !std::path::Path::new(&self.cert_path).exists() {
                return Err(anyhow::anyhow!(
                    "TLS certificate file not found: {}",
                    self.cert_path
                ));
            }
            if !std::path::Path::new(&self.key_path).exists() {
                return Err(anyhow::anyhow!(
                    "TLS key file not found: {}",
                    self.key_path
                ));
            }
            Ok(())
        }
    }

    pub struct WebState {
        pub namespace: String,
        pub kube_client: KubeClient,
        pub api_key: Option<String>,
        pub auth: crate::api::auth::SharedAuth,
        pub kryton: Option<crate::kryton::Client>,
        rate_limiter: RateLimiterState,
    }

    impl WebState {
        pub async fn new(namespace: String, rate_limit_per_minute: u64) -> anyhow::Result<Self> {
            let auth = Arc::new(crate::api::auth::AuthState::from_env()?);
            let api_key = auth.api_key.clone();
            if api_key.is_none() && std::env::var("ZORVIA_ADMIN_PASSWORD").is_err() {
                log::warn!(
                    "Neither ZORVIA_API_KEY nor custom ZORVIA_ADMIN_PASSWORD set; \
                     using lab default admin/Admin@321 for login"
                );
            }
            let kube_client = KubeClient::new().await?;
            let kryton = crate::kryton::Client::from_env()?;
            if let Some(ref client) = kryton {
                log::info!("Kryton integration enabled: {} (project={:?})", client.base_url(), client.configured_project());
            }
            Ok(Self {
                namespace,
                kube_client,
                api_key,
                auth,
                kryton,
                rate_limiter: RateLimiterState::new(rate_limit_per_minute, 60),
            })
        }

        pub fn client(&self) -> KubeClient {
            self.kube_client.clone()
        }
    }

    pub type SharedState = Arc<RwLock<WebState>>;

    // ── Kubernetes name validation (RFC 1123 DNS label) ──────────

    /// Validate that a string is a valid Kubernetes name (RFC 1123 DNS label).
    /// Must be at most 63 characters, consist of lowercase alphanumeric characters
    /// or '-', and must start and end with an alphanumeric character.
    fn is_valid_k8s_name(s: &str) -> bool {
        if s.is_empty() || s.len() > 63 {
            return false;
        }
        let bytes = s.as_bytes();
        // Must start and end with alphanumeric
        if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
            return false;
        }
        if !bytes[bytes.len() - 1].is_ascii_lowercase()
            && !bytes[bytes.len() - 1].is_ascii_digit()
        {
            return false;
        }
        // All characters must be lowercase alphanumeric or '-'
        s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    }

    /// Return a 400 error if a path parameter is not a valid Kubernetes name.
    fn validate_k8s_params(
        params: &[(&str, &str)],
    ) -> Option<(StatusCode, Json<serde_json::Value>)> {
        for (label, value) in params {
            if !is_valid_k8s_name(value) {
                return Some(err_json(
                    400,
                    "INVALID_PARAMETER",
                    &format!("'{}' is not a valid Kubernetes name", label),
                ));
            }
        }
        None
    }

    // ── Auth middleware ──────────────────────────────────────────

    /// Authentication middleware: JWT Bearer, or shared API key.
    /// Public: health, auth login/OIDC, static SPA/dashboard assets.
    async fn auth_middleware(
        State(state): State<SharedState>,
        headers: HeaderMap,
        request: axum::extract::Request,
        next: middleware::Next,
    ) -> impl IntoResponse {
        let path = request.uri().path().to_string();

        if is_public_path(&path) || request.method() == axum::http::Method::OPTIONS {
            return next.run(request).await.into_response();
        }

        let s = state.read().await;

        if let Some(key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
            if s.auth.api_key_ok(key) {
                drop(s);
                return next.run(request).await.into_response();
            }
        }

        if let Some(token) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        {
            if s.auth.api_key_ok(token) || s.auth.validate_bearer(token).is_some() {
                drop(s);
                return next.run(request).await.into_response();
            }
        }

        drop(s);
        let (status, json) = err_json(401, "UNAUTHORIZED", "Invalid or missing credentials");
        (status, json).into_response()
    }

    fn is_public_path(path: &str) -> bool {
        // SPA assets and client routes are public; only /api/* is gated (except auth/health/instance).
        if !path.starts_with("/api/") {
            return true;
        }
        matches!(
            path,
            "/api/v1/health"
                | "/api/health"
                | "/api/readyz"
                | "/api/metrics"
                | "/api/v1/auth/login"
                | "/api/v1/auth/providers"
                | "/api/v1/instance"
                | "/api/instance"
        ) || path.starts_with("/api/v1/auth/oidc/")
    }

    // ── Rate limiting middleware ──────────────────────────────────

    /// Middleware that enforces a global request rate limit.
    async fn rate_limit_middleware(
        State(state): State<SharedState>,
        request: axum::extract::Request,
        next: middleware::Next,
    ) -> impl IntoResponse {
        let path = request.uri().path().to_string();
        // Never rate-limit SPA assets or public health; only gate API traffic.
        if !path.starts_with("/api/")
            || path == "/api/v1/health"
            || path == "/api/health"
            || path == "/api/readyz"
            || path == "/api/metrics"
        {
            return next.run(request).await.into_response();
        }

        let s = state.read().await;
        if !s.rate_limiter.check_rate_limit() {
            let (status, json) = err_json(429, "RATE_LIMITED", "Too many requests. Please slow down.");
            return (status, json).into_response();
        }
        drop(s);
        next.run(request).await.into_response()
    }

    // ── Security headers middleware ─────────────────────────────

    /// Middleware that adds security headers to every response.
    async fn security_headers_middleware(
        request: axum::extract::Request,
        next: middleware::Next,
    ) -> impl IntoResponse {
        let mut response = next.run(request).await;
        let headers = response.headers_mut();
        headers.insert("x-content-type-options", "nosniff".parse().unwrap());
        headers.insert("x-frame-options", "DENY".parse().unwrap());
        headers.insert("cache-control", "no-store".parse().unwrap());
        headers.insert("x-xss-protection", "0".parse().unwrap());
        response
    }

    // ── CORS configuration ──────────────────────────────────────

    /// Build a CORS layer. If `ZORVIA_CORS_ORIGINS` is set (comma-separated
    /// list of origins), allow those origins. Otherwise default to same-origin
    /// only (no extra origins allowed).
    fn build_cors_layer() -> CorsLayer {
        let origins = std::env::var("ZORVIA_CORS_ORIGINS").ok();

        let allow_origin = match origins {
            Some(ref raw) if !raw.is_empty() => {
                let parsed: Vec<_> = raw
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
                if parsed.is_empty() {
                    AllowOrigin::default()
                } else {
                    AllowOrigin::list(parsed)
                }
            }
            _ => AllowOrigin::default(), // same-origin: no Access-Control-Allow-Origin header
        };

        CorsLayer::new()
            .allow_origin(allow_origin)
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::PATCH,
                axum::http::Method::DELETE,
            ])
            .allow_headers([
                header::CONTENT_TYPE,
                header::AUTHORIZATION,
                "x-api-key".parse().unwrap(),
            ])
    }

    // ── Router ──────────────────────────────────────────────────

    fn resolve_web_dir() -> std::path::PathBuf {
        if let Ok(dir) = std::env::var("ZORVIA_WEB_DIR") {
            return std::path::PathBuf::from(dir);
        }
        let candidates = [
            std::path::PathBuf::from("/usr/share/zorvia/web"),
            std::path::PathBuf::from("web/dist"),
            std::path::PathBuf::from("./web/dist"),
        ];
        for c in candidates {
            if c.join("index.html").exists() {
                return c;
            }
        }
        std::path::PathBuf::from("/usr/share/zorvia/web")
    }

    pub fn build_router(state: SharedState) -> Router {
        let web_dir = resolve_web_dir();
        log::info!("Serving SPA from {}", web_dir.display());
        let index = web_dir.join("index.html");
        let spa = ServeDir::new(web_dir).fallback(ServeFile::new(index));

        let api = Router::new()
            // Auth (Zorvia)
            .route("/v1/auth/login", post(auth_login))
            .route("/v1/auth/me", get(auth_me))
            .route("/v1/auth/providers", get(auth_providers))
            .route("/v1/auth/totp/setup", post(auth_totp_setup))
            .route("/v1/auth/totp/verify", post(auth_totp_verify))
            .route("/v1/auth/totp/disable", post(auth_totp_disable))
            .route("/v1/auth/oidc/callback", get(auth_oidc_callback))
            .route("/v1/auth/oidc/:id", get(auth_oidc_login))
            .route("/v1/users", get(users_list).post(users_create))
            .route("/v1/users/:id", delete(users_delete))
            .route("/v1/users/:id/role", put(users_update_role))
            .route("/v1/users/:id/enabled", put(users_set_enabled))
            .route("/v1/instance", get(instance_handler))
            .route("/instance", get(instance_handler))
            // Images (create wizard)
            .route("/images", get(fabric_list_images))
            .route("/images/cloud", get(fabric_list_cloud_images))
            .route("/images/downloads", get(fabric_list_downloads))
            .route("/images/cloud/download", post(fabric_start_download))
            // Fabric-compat VM API (unwrapped JSON)
            .route("/vms", get(fabric_list_vms).post(fabric_create_vm))
            .route("/vms/:name", get(fabric_get_vm).delete(fabric_delete_vm))
            .route("/vms/:name/start", post(fabric_start_vm))
            .route("/vms/:name/stop", post(fabric_stop_vm))
            .route("/vms/:name/restart", post(fabric_restart_vm))
            .route("/vms/:name/pause", post(fabric_pause_vm))
            .route("/vms/:name/resume", post(fabric_resume_vm))
            .route("/vms/:name/hotplug/cpu", post(fabric_hotplug_cpu))
            .route("/vms/:name/hotplug/memory", post(fabric_hotplug_memory))
            .route("/vms/:name/hotplug/disk", post(fabric_hotplug_disk))
            .route(
                "/vms/:name/hotplug/disk/:device_id",
                delete(fabric_hotunplug_disk),
            )
            .route("/vms/:name/hotplug/nic", post(fabric_hotplug_nic))
            .route(
                "/vms/:name/hotplug/nic/:device_id",
                delete(fabric_hotunplug_nic),
            )
            .route("/vms/:name/drift", post(fabric_vm_drift))
            .route("/vms/:name/plan", post(fabric_vm_change_plan))
            .route("/vms/:name/disks", get(fabric_list_disks))
            .route("/vms/:name/disks/:disk_name/resize", post(fabric_resize_disk))
            .route("/vms/:name/interfaces", get(fabric_list_interfaces))
            .route("/vms/:name/migrate", post(fabric_migrate_vm))
            .route("/vms/:name/migrations", get(fabric_list_vm_migrations))
            .route("/migrations/:id", get(fabric_get_migration))
            .route("/migrations/:id/cancel", post(fabric_cancel_migration))
            // Rook-Ceph distributed storage
            .route("/storage/rook/bootstrap", post(rook_bootstrap))
            .route("/storage/rook/cluster", get(rook_cluster_status).post(rook_create_cluster).delete(rook_delete_cluster))
            .route("/storage/rook/pools", get(rook_list_pools).post(rook_create_pool))
            .route("/storage/rook/pools/:name", delete(rook_delete_pool))
            .route("/storage/rook/filesystems", get(rook_list_filesystems).post(rook_create_filesystem))
            .route("/storage/rook/filesystems/:name", delete(rook_delete_filesystem))
            .route("/storage/rook/objectstores", get(rook_list_object_stores).post(rook_create_object_store))
            .route("/storage/rook/objectstores/:name", delete(rook_delete_object_store))
            .route("/storage/rook/storage-classes", post(rook_create_storage_class))
            .route("/storage/rook/volume-snapshot-classes", post(rook_create_volume_snapshot_class))
            .route("/vms/:name/metrics", get(fabric_vm_metrics))
            .route("/vms/:name/guest-insight", get(fabric_guest_insight))
            .route("/vms/:name/wait-ready", post(fabric_wait_guest_ready))
            .route("/vms/:name/logs", get(fabric_vm_logs))
            .route("/datavolumes/:name/wait", post(fabric_wait_data_volume))
            .route("/readyz", get(fabric_readyz))
            .route("/metrics", get(fabric_prom_metrics))
            .route("/vms/:name/port-forwards", post(fabric_add_port_forward))
            .route(
                "/vms/:name/port-forwards/:host_port",
                delete(fabric_remove_port_forward),
            )
            .route("/vms/:name/cloud-init", post(fabric_cloud_init))
            .route("/vms/:name/clone", post(fabric_clone_vm))
            .route(
                "/vms/:name/snapshots",
                get(fabric_list_vm_snapshots).post(fabric_create_snapshot),
            )
            .route(
                "/vms/:name/snapshots/:id",
                delete(fabric_delete_snapshot),
            )
            .route(
                "/vms/:name/snapshots/:id/revert",
                post(fabric_revert_snapshot),
            )
            .route("/snapshots", get(fabric_list_snapshots))
            .route("/events", get(fabric_list_events))
            .route("/events/stream", get(fabric_events_stream))
            .route("/capabilities", get(fabric_capabilities))
            .route("/health", get(fabric_health))
            .route("/dashboard/overview", get(fabric_overview))
            // Native Zorvia v1 API
            .route("/v1/vms", get(list_vms_handler))
            .route("/v1/vms/:ns/:name", get(get_vm_handler))
            .route("/v1/vms/:ns/:name", delete(delete_vm_handler))
            .route("/v1/vms/:ns/:name/start", post(start_vm_handler))
            .route("/v1/vms/:ns/:name/stop", post(stop_vm_handler))
            .route("/v1/vms/:ns/:name/restart", post(restart_vm_handler))
            .route("/v1/snapshots", get(list_snapshots_handler))
            .route("/v1/snapshots/:ns/:vm", get(list_vm_snapshots_handler))
            .route(
                "/v1/snapshots/:ns/:name/delete",
                post(delete_snapshot_handler),
            )
            .route("/v1/events", get(list_events_handler))
            .route("/v1/events/recent", get(recent_events_handler))
            .route("/v1/dashboard/overview", get(dashboard_overview_handler))
            // Kryton Windows control plane (server-side token; Zorvia auth at edge)
            .route("/v1/kryton/status", get(kryton_status))
            .route("/v1/kryton/capabilities", get(kryton_capabilities))
            .route("/v1/kryton/doctor", get(kryton_doctor))
            .route("/v1/kryton/images", get(kryton_images))
            .route("/v1/kryton/summary", get(kryton_summary))
            .route("/v1/kryton/machines", get(kryton_list_machines).post(kryton_create_machine))
            .route("/v1/kryton/machines/:id", get(kryton_get_machine).delete(kryton_delete_machine))
            .route("/v1/kryton/machines/:id/start", post(kryton_start_machine))
            .route("/v1/kryton/machines/:id/stop", post(kryton_stop_machine))
            .route("/v1/kryton/machines/:id/snapshot", post(kryton_snapshot_machine))
            .route("/v1/kryton/machines/:id/snapshots", get(kryton_list_snapshots))
            .route("/v1/kryton/machines/:id/snapshots/:sid/restore", post(kryton_restore_snapshot))
            .route("/v1/kryton/machines/:id/snapshots/:sid", delete(kryton_delete_snapshot))
            .route("/v1/health", get(health_handler))
            .fallback(fabric_not_implemented)
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                std::time::Duration::from_secs(30),
            ))
            .with_state(state.clone());

        Router::new()
            .route("/dashboard", get(|| async { axum::response::Redirect::temporary("/app") }))
            .route("/ws/console/:name", get(ws_console))
            .route("/ws/vnc/:name", get(ws_vnc))
            .route("/ws/ssh/:name", get(ws_ssh))
            .nest("/api", api)
            .fallback_service(spa)
            .layer(middleware::from_fn(security_headers_middleware))
            .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .layer(build_cors_layer())
            .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
            .with_state(state)
    }

    pub async fn start_server(
        host: &str,
        port: u16,
        namespace: String,
        tls_config: Option<TlsConfig>,
        rate_limit_per_minute: u64,
    ) -> anyhow::Result<()> {
        // Validate TLS config early if provided
        if let Some(ref tls) = tls_config {
            tls.validate()?;
        }

        // Initialize kube client at startup instead of lazily per-request
        let state = Arc::new(RwLock::new(WebState::new(namespace, rate_limit_per_minute).await?));
        let app = build_router(state);
        let addr = format!("{}:{}", host, port);

        if let Some(tls) = tls_config {
            log::info!("Starting HTTPS server on {}", addr);

            // Install the ring crypto provider (already used by kube-client)
            let _ = rustls::crypto::ring::default_provider().install_default();

            let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
                &tls.cert_path,
                &tls.key_path,
            )
            .await?;
            let addr: std::net::SocketAddr = addr.parse()?;
            axum_server::bind_rustls(addr, rustls_config)
                .serve(app.into_make_service())
                .await?;
        } else {
            log::info!("Starting HTTP server on {}", addr);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;
        }

        Ok(())
    }

    async fn auth_shared(state: &SharedState) -> crate::api::auth::SharedAuth {
        state.read().await.auth.clone()
    }

    async fn auth_login(
        State(state): State<SharedState>,
        Json(body): Json<crate::api::auth::handlers::LoginRequest>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::login_handler(axum::extract::State(auth), Json(body)).await
    }

    async fn auth_me(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::me_handler(axum::extract::State(auth), headers).await
    }

    async fn auth_providers(State(state): State<SharedState>) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::providers_handler(axum::extract::State(auth)).await
    }

    async fn auth_totp_setup(
        State(state): State<SharedState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::totp_setup_handler(axum::extract::State(auth), headers).await
    }

    async fn auth_totp_verify(
        State(state): State<SharedState>,
        headers: HeaderMap,
        Json(body): Json<crate::api::auth::handlers::TotpVerifyRequest>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::totp_verify_handler(axum::extract::State(auth), headers, Json(body)).await
    }

    async fn auth_totp_disable(
        State(state): State<SharedState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::totp_disable_handler(axum::extract::State(auth), headers).await
    }

    async fn auth_oidc_login(
        State(state): State<SharedState>,
        Path(id): Path<String>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::oidc_login_handler(axum::extract::State(auth), Path(id)).await
    }

    async fn auth_oidc_callback(
        State(state): State<SharedState>,
        Query(q): Query<crate::api::auth::handlers::OidcCallbackQuery>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::oidc_callback_handler(axum::extract::State(auth), Query(q)).await
    }

    // ── Admin-only user management ──────────────────────────────────────
    // Every handler here re-validates the bearer token itself and requires
    // Role::Admin -- there is no role-checking middleware in this server,
    // `is_public_path` only distinguishes authenticated vs. not.

    async fn users_list(State(state): State<SharedState>, headers: HeaderMap) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::handlers::list_users_handler(axum::extract::State(auth), headers).await
    }

    async fn users_create(
        State(state): State<SharedState>,
        headers: HeaderMap,
        Json(body): Json<crate::api::auth::handlers::CreateUserRequest>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::handlers::create_user_handler(axum::extract::State(auth), headers, Json(body))
            .await
    }

    async fn users_delete(
        State(state): State<SharedState>,
        headers: HeaderMap,
        Path(id): Path<String>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::handlers::delete_user_handler(axum::extract::State(auth), headers, Path(id))
            .await
    }

    async fn users_update_role(
        State(state): State<SharedState>,
        headers: HeaderMap,
        Path(id): Path<String>,
        Json(body): Json<crate::api::auth::handlers::UpdateRoleRequest>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::handlers::update_role_handler(
            axum::extract::State(auth),
            headers,
            Path(id),
            Json(body),
        )
        .await
    }

    async fn users_set_enabled(
        State(state): State<SharedState>,
        headers: HeaderMap,
        Path(id): Path<String>,
        Json(body): Json<crate::api::auth::handlers::SetEnabledRequest>,
    ) -> impl IntoResponse {
        let auth = auth_shared(&state).await;
        crate::api::auth::handlers::set_enabled_handler(
            axum::extract::State(auth),
            headers,
            Path(id),
            Json(body),
        )
        .await
    }

    async fn instance_handler() -> impl IntoResponse {
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("HOST"))
            .unwrap_or_else(|_| "zorvia".into());
        Json(serde_json::json!({
            "product": "Zorvia",
            "product_id": "zorvia",
            "version": env!("CARGO_PKG_VERSION"),
            "hostname": hostname,
            "deploy_mode": "kubernetes",
            "deploy_label": "Kubernetes · NodePort 30152",
            "kubernetes": true,
            "kubernetes_namespace": "zorvia-system",
            "listen": ":30152",
        }))
    }

    async fn fabric_health() -> impl IntoResponse {
        Json(serde_json::json!({
            "status": "healthy",
            "service": "zorvia-api",
            "version": env!("CARGO_PKG_VERSION"),
        }))
    }

    async fn fabric_readyz(State(state): State<SharedState>) -> impl IntoResponse {
        let s = state.read().await;
        match s.kube_client.list_vms(&s.namespace).await {
            Ok(_) => (
                StatusCode::OK,
                Json(serde_json::json!({"status": "ready", "kube": true})),
            )
                .into_response(),
            Err(e) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "status": "not-ready",
                    "kube": false,
                    "error": sanitize_error(&e),
                })),
            )
                .into_response(),
        }
    }

    async fn fabric_prom_metrics() -> impl IntoResponse {
        let body = format!(
            "# HELP zorvia_up 1 if the API process is running\n# TYPE zorvia_up gauge\nzorvia_up 1\n# HELP zorvia_build_info Build version\n# TYPE zorvia_build_info gauge\nzorvia_build_info{{version=\"{}\"}} 1\n",
            env!("CARGO_PKG_VERSION")
        );
        (
            [(
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            body,
        )
    }

    async fn fabric_not_implemented() -> impl IntoResponse {
        (
            StatusCode::NOT_IMPLEMENTED,
            Json(serde_json::json!({
                "success": false,
                "status": 501,
                "error": {
                    "code": "NOT_IMPLEMENTED",
                    "message": "This API is not available on Zorvia yet"
                },
                "data": null
            })),
        )
    }

    fn fabric_map_state(status: &str) -> &'static str {
        match status.to_lowercase().as_str() {
            s if s.contains("run") => "running",
            s if s.contains("stop") => "stopped",
            s if s.contains("paus") => "paused",
            s if s.contains("start") || s.contains("provision") || s.contains("wait") => "starting",
            s if s.contains("fail") || s.contains("error") => "failed",
            _ => "unknown",
        }
    }

    fn fabric_vm_json(info: &VmInfo) -> serde_json::Value {
        let cpus = info
            .cpu
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1);
        let mem_s = info.memory.trim().to_lowercase();
        let memory = if let Some(n) = mem_s.strip_suffix("gi") {
            (n.trim().parse::<f64>().unwrap_or(1.0) * 1024.0) as u64
        } else if let Some(n) = mem_s.strip_suffix('g') {
            (n.trim().parse::<f64>().unwrap_or(1.0) * 1024.0) as u64
        } else if let Some(n) = mem_s.strip_suffix("mi") {
            n.trim().parse().unwrap_or(1024)
        } else {
            1024
        };
        let ip = if info.ip.is_empty() || info.ip == "N/A" {
            serde_json::Value::Null
        } else {
            serde_json::json!(info.ip)
        };
        serde_json::json!({
            "name": info.name,
            "state": fabric_map_state(&info.status),
            "cpus": cpus,
            "memory": memory,
            "image": info.disk,
            "ip": ip,
        })
    }

    async fn fabric_list_vms(State(state): State<SharedState>) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        match client.list_vms(&namespace).await {
            Ok(vms) => {
                let mut out = Vec::new();
                for vm in &vms {
                    let name = vm.metadata.name.clone().unwrap_or_default();
                    let ip = client.get_vm_ip(&namespace, &name).await.unwrap_or(None);
                    out.push(fabric_vm_json(&VmInfo::from_vm_with_ip(vm, ip)));
                }
                Json(out).into_response()
            }
            Err(e) => {
                let (st, j) = err_json(500, "LIST_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_get_vm(
        State(state): State<SharedState>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        match client.get_vm(&namespace, &name).await {
            Ok(vm) => {
                let ip = client.get_vm_ip(&namespace, &name).await.unwrap_or(None);
                let mut body = fabric_vm_json(&VmInfo::from_vm_with_ip(&vm, ip));
                if let Ok(pfs) = client.list_port_forwards(&namespace, &name).await {
                    if let Some(obj) = body.as_object_mut() {
                        obj.insert(
                            "port_forwards".into(),
                            serde_json::json!(pfs
                                .iter()
                                .map(|p| serde_json::json!({
                                    "host_port": p.host_port,
                                    "guest_port": p.guest_port,
                                    "protocol": p.protocol,
                                    "expose_host": p.expose_host,
                                }))
                                .collect::<Vec<_>>()),
                        );
                    }
                }
                Json(body).into_response()
            }
            Err(e) => {
                let (st, j) = err_json(404, "NOT_FOUND", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_start_vm(
        State(state): State<SharedState>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        match client.start_vm(&namespace, &name).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => {
                let (st, j) = err_json(500, "START_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_stop_vm(
        State(state): State<SharedState>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        match client.stop_vm(&namespace, &name).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => {
                let (st, j) = err_json(500, "STOP_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_restart_vm(
        State(state): State<SharedState>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        match client.restart_vm(&namespace, &name).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => {
                let (st, j) = err_json(500, "RESTART_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_delete_vm(
        State(state): State<SharedState>,
        Path(name): Path<String>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        let _ = client.delete_vm_port_forwards(&namespace, &name).await;
        match client.delete_vm(&namespace, &name).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => {
                let (st, j) = err_json(500, "DELETE_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_list_snapshots(State(state): State<SharedState>) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        drop(s);
        match crate::snapshots::SnapshotManager::new(&namespace).await {
            Ok(manager) => match manager.list_all_snapshots().await {
                Ok(snapshots) => {
                    let items: Vec<_> = snapshots
                        .into_iter()
                        .map(|s| {
                            serde_json::json!({
                                "id": s.name,
                                "vm_name": s.vm_name,
                                "name": s.name,
                                "description": null,
                                "snapshot_type": "Disk",
                                "parent_id": null,
                                "size_bytes": 0,
                                "created": "",
                            })
                        })
                        .collect();
                    Json(items).into_response()
                }
                Err(e) => {
                    let (st, j) = err_json(500, "LIST_FAILED", &sanitize_error(&e));
                    (st, j).into_response()
                }
            },
            Err(e) => {
                let (st, j) = err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_list_vm_snapshots(
        State(state): State<SharedState>,
        Path(vm): Path<String>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        drop(s);
        match crate::snapshots::SnapshotManager::new(&namespace).await {
            Ok(manager) => match manager.list_snapshots_for_vm(&vm).await {
                Ok(snapshots) => {
                    let items: Vec<_> = snapshots
                        .into_iter()
                        .map(|s| {
                            serde_json::json!({
                                "id": s.name,
                                "vm_name": vm,
                                "name": s.name,
                                "description": null,
                                "snapshot_type": "Disk",
                                "parent_id": null,
                                "size_bytes": 0,
                                "created": "",
                            })
                        })
                        .collect();
                    Json(items).into_response()
                }
                Err(e) => {
                    let (st, j) = err_json(500, "LIST_FAILED", &sanitize_error(&e));
                    (st, j).into_response()
                }
            },
            Err(e) => {
                let (st, j) = err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    async fn fabric_list_events(State(state): State<SharedState>) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        use k8s_openapi::api::core::v1::Event;
        use kube::Api;
        let events_api: Api<Event> = Api::namespaced(client.client(), &namespace);
        let lp = kube::api::ListParams::default().limit(100);
        match events_api.list(&lp).await {
            Ok(event_list) => {
                let items: Vec<_> = event_list
                    .items
                    .into_iter()
                    .map(|e| {
                        serde_json::json!({
                            "type": e.type_.unwrap_or_else(|| "Normal".into()),
                            "reason": e.reason.unwrap_or_default(),
                            "message": e.message.unwrap_or_default(),
                            "object": e.involved_object.name.unwrap_or_default(),
                            "timestamp": e.last_timestamp
                                .map(|t| t.0.to_rfc3339())
                                .or_else(|| e.metadata.creation_timestamp.map(|t| t.0.to_rfc3339()))
                                .unwrap_or_default(),
                        })
                    })
                    .collect();
                Json(items).into_response()
            }
            Err(e) => {
                let (st, j) = err_json(500, "LIST_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    /// Live SSE stream of cluster `Event` objects, shaped for `useEventStream.ts`.
    async fn fabric_events_stream(State(state): State<SharedState>) -> impl IntoResponse {
        use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
        use futures_util::StreamExt;
        use k8s_openapi::api::core::v1::Event as K8sEvent;
        use kube::runtime::{watcher, WatchStreamExt};
        use kube::Api;
        use std::convert::Infallible;

        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        drop(s);

        let api: Api<K8sEvent> = Api::namespaced(client.client(), &namespace);
        let stream = watcher(api, watcher::Config::default())
            .applied_objects()
            .filter_map(|res| async move { res.ok() })
            .map(|e| {
                let payload = serde_json::json!({
                    "id": e.metadata.uid.clone().unwrap_or_default(),
                    "event_type": e.reason.clone().unwrap_or_else(|| "Normal".into()),
                    "vm_name": e.involved_object.name.clone().unwrap_or_default(),
                    "detail": e.message.clone(),
                    "timestamp": e.last_timestamp
                        .as_ref()
                        .map(|t| t.0.to_rfc3339())
                        .or_else(|| {
                            e.metadata.creation_timestamp.as_ref().map(|t| t.0.to_rfc3339())
                        })
                        .unwrap_or_default(),
                });
                Ok::<_, Infallible>(SseEvent::default().event("vm-event").data(payload.to_string()))
            });

        Sse::new(stream).keep_alive(KeepAlive::default())
    }

    /// Lightweight live-reachability probe for each subsystem the "offline" pill and
    /// capabilities page care about.
    async fn fabric_capabilities(State(state): State<SharedState>) -> impl IntoResponse {
        use k8s_openapi::api::core::v1::PersistentVolumeClaim;
        use k8s_openapi::api::networking::v1::NetworkPolicy;
        use kube::Api;

        fn phase(ok: bool, err: Option<String>) -> serde_json::Value {
            serde_json::json!({ "phase": if ok { "live" } else { "unreachable" }, "detail": err })
        }

        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        let auth_configured =
            s.api_key.is_some() || std::env::var("ZORVIA_ADMIN_PASSWORD").is_ok();
        drop(s);

        let vm_driver = match client.list_vms(&namespace).await {
            Ok(_) => phase(true, None),
            Err(e) => phase(false, Some(sanitize_error(&e))),
        };

        let events = {
            use k8s_openapi::api::core::v1::Event;
            let api: Api<Event> = Api::namespaced(client.client(), &namespace);
            match api.list(&kube::api::ListParams::default().limit(1)).await {
                Ok(_) => phase(true, None),
                Err(e) => phase(false, Some(sanitize_error(&e))),
            }
        };

        let storage = {
            let api: Api<PersistentVolumeClaim> = Api::namespaced(client.client(), &namespace);
            match api.list(&kube::api::ListParams::default().limit(1)).await {
                Ok(_) => phase(true, None),
                Err(e) => phase(false, Some(sanitize_error(&e))),
            }
        };

        let network_security = {
            let api: Api<NetworkPolicy> = Api::namespaced(client.client(), &namespace);
            match api.list(&kube::api::ListParams::default().limit(1)).await {
                Ok(_) => phase(true, None),
                Err(e) => phase(false, Some(sanitize_error(&e))),
            }
        };

        let auth_phase = if auth_configured { "live" } else { "off" };
        let auth = serde_json::json!({ "phase": auth_phase, "detail": null });

        Json(serde_json::json!({
            "vm_driver": vm_driver,
            "storage": storage,
            "network_security": network_security,
            "vm_dataplane": network_security,
            "auth": auth,
            "events": events,
            "hubble_ui_url": std::env::var("ZORVIA_HUBBLE_UI_URL").ok(),
        }))
    }

    async fn fabric_overview(State(state): State<SharedState>) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();
        match client.list_vms(&namespace).await {
            Ok(vms) => {
                let mut running = 0u32;
                let mut stopped = 0u32;
                let mut failed = 0u32;
                let total = vms.len() as u32;
                for vm in &vms {
                    let status = vm
                        .status
                        .as_ref()
                        .and_then(|st| st.printable_status.as_deref())
                        .unwrap_or("Unknown");
                    match fabric_map_state(status) {
                        "running" => running += 1,
                        "failed" => failed += 1,
                        _ => stopped += 1,
                    }
                }
                Json(serde_json::json!({
                    "vms": { "total": total, "running": running, "stopped": stopped, "failed": failed },
                    "namespace": namespace,
                }))
                .into_response()
            }
            Err(e) => {
                let (st, j) = err_json(500, "LIST_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        }
    }

    fn req_ctx(method: HttpMethod, path: &str) -> RequestContext {
        RequestContext::new(method, path)
    }


    /// Sanitize internal error details before sending to clients.
    ///
    /// For known patterns (e.g. kube errors), returns just the first sentence.
    /// For anything else, returns a generic message to avoid leaking internals.
    fn sanitize_error(e: &dyn std::fmt::Display) -> String {
        let msg = e.to_string();
        // Known patterns where the first sentence is safe to expose
        let known_prefixes = [
            "ApiError",
            "NotFound",
            "Conflict",
            "Unauthorized",
            "Forbidden",
            "Timeout",
            "connection",
        ];
        let is_known = known_prefixes
            .iter()
            .any(|p| msg.starts_with(p));

        if is_known {
            // Keep the first sentence, bounded to a safe length. Cutting at
            // the first ": " (as this used to do) is wrong: kube-rs's
            // ApiError Display starts "ApiError: <reason>", so the very
            // first ": " is the one separating the prefix from the actual
            // reason — truncating there dropped 100% of the useful detail,
            // leaving clients with just the literal word "ApiError" and no
            // way to diagnose a failed request. Sentence-end (". ") is a
            // safe cut point since it only appears after real content.
            let mut end = msg.find(". ").unwrap_or(msg.len()).min(500);
            while end > 0 && !msg.is_char_boundary(end) {
                end -= 1;
            }
            msg[..end].to_string()
        } else {
            "Internal server error".to_string()
        }
    }

    // ── VM Endpoints ──────────────────────────────────────────────

    #[derive(Deserialize)]
    pub struct VmQuery {
        pub namespace: Option<String>,
    }

    async fn list_vms_handler(
        State(state): State<SharedState>,
        Query(query): Query<VmQuery>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = query.namespace.clone().unwrap_or_else(|| s.namespace.clone());
        let client = s.client();

        match client.list_vms(&namespace).await {
            Ok(vms) => {
                let mut vm_infos = Vec::with_capacity(vms.len());
                for vm in &vms {
                    let name = vm.metadata.name.clone().unwrap_or_default();
                    let ip = client.get_vm_ip(&namespace, &name).await.unwrap_or(None);
                    vm_infos.push(VmInfo::from_vm_with_ip(vm, ip));
                }
                let ctx = req_ctx(HttpMethod::GET, "/api/v1/vms");
                ok_json(&ApiResponse::success(&vm_infos, &ctx.request_id))
            }
            Err(e) => err_json(500, "INTERNAL_ERROR", &sanitize_error(&e)),
        }
    }

    async fn get_vm_handler(
        State(state): State<SharedState>,
        Path((ns, name)): Path<(String, String)>,
    ) -> impl IntoResponse {
        if let Some(resp) = validate_k8s_params(&[("namespace", &ns), ("name", &name)]) {
            return resp;
        }

        let s = state.read().await;
        let client = s.client();

        match client.get_vm(&ns, &name).await {
            Ok(vm) => {
                let ip = client.get_vm_ip(&ns, &name).await.unwrap_or(None);
                let info = VmInfo::from_vm_with_ip(&vm, ip);

                // Also fetch VMI details if running
                let vmi_detail = client.get_vmi(&ns, &name).await.ok();
                let detail = VmDetail {
                    info,
                    vmi_status: vmi_detail.and_then(|v| v.status),
                };

                let ctx = req_ctx(HttpMethod::GET, "/api/v1/vms/:ns/:name");
                ok_json(&ApiResponse::success(&detail, &ctx.request_id))
            }
            Err(e) => {
                let msg = sanitize_error(&e);
                if msg.contains("NotFound") || msg.contains("not found") {
                    err_json(404, "NOT_FOUND", &format!("VM '{}' not found", name))
                } else {
                    err_json(500, "INTERNAL_ERROR", &msg)
                }
            }
        }
    }

    async fn start_vm_handler(
        State(state): State<SharedState>,
        Path((ns, name)): Path<(String, String)>,
    ) -> impl IntoResponse {
        if let Some(resp) = validate_k8s_params(&[("namespace", &ns), ("name", &name)]) {
            return resp;
        }

        let s = state.read().await;
        let client = s.client();

        match client.start_vm(&ns, &name).await {
            Ok(_) => {
                let ctx = req_ctx(HttpMethod::POST, "/api/v1/vms/:ns/:name/start");
                ok_json(&ApiResponse::success(
                    &serde_json::json!({"message": format!("VM '{}' started", name)}),
                    &ctx.request_id,
                ))
            }
            Err(e) => err_json(500, "START_FAILED", &sanitize_error(&e)),
        }
    }

    async fn stop_vm_handler(
        State(state): State<SharedState>,
        Path((ns, name)): Path<(String, String)>,
    ) -> impl IntoResponse {
        if let Some(resp) = validate_k8s_params(&[("namespace", &ns), ("name", &name)]) {
            return resp;
        }

        let s = state.read().await;
        let client = s.client();

        match client.stop_vm(&ns, &name).await {
            Ok(_) => {
                let ctx = req_ctx(HttpMethod::POST, "/api/v1/vms/:ns/:name/stop");
                ok_json(&ApiResponse::success(
                    &serde_json::json!({"message": format!("VM '{}' stopped", name)}),
                    &ctx.request_id,
                ))
            }
            Err(e) => err_json(500, "STOP_FAILED", &sanitize_error(&e)),
        }
    }

    async fn restart_vm_handler(
        State(state): State<SharedState>,
        Path((ns, name)): Path<(String, String)>,
    ) -> impl IntoResponse {
        if let Some(resp) = validate_k8s_params(&[("namespace", &ns), ("name", &name)]) {
            return resp;
        }

        let s = state.read().await;
        let client = s.client();

        match client.restart_vm(&ns, &name).await {
            Ok(_) => {
                let ctx = req_ctx(HttpMethod::POST, "/api/v1/vms/:ns/:name/restart");
                ok_json(&ApiResponse::success(
                    &serde_json::json!({"message": format!("VM '{}' restarted", name)}),
                    &ctx.request_id,
                ))
            }
            Err(e) => err_json(500, "RESTART_FAILED", &sanitize_error(&e)),
        }
    }

    async fn delete_vm_handler(
        State(state): State<SharedState>,
        Path((ns, name)): Path<(String, String)>,
    ) -> impl IntoResponse {
        if let Some(resp) = validate_k8s_params(&[("namespace", &ns), ("name", &name)]) {
            return resp;
        }

        let s = state.read().await;
        let client = s.client();

        match client.delete_vm(&ns, &name).await {
            Ok(_) => {
                let ctx = req_ctx(HttpMethod::DELETE, "/api/v1/vms/:ns/:name");
                ok_json(&ApiResponse::success(
                    &serde_json::json!({"message": format!("VM '{}' deleted", name)}),
                    &ctx.request_id,
                ))
            }
            Err(e) => err_json(500, "DELETE_FAILED", &sanitize_error(&e)),
        }
    }

    // ── Snapshot Endpoints ────────────────────────────────────────

    async fn list_snapshots_handler(
        State(state): State<SharedState>,
        Query(query): Query<VmQuery>,
    ) -> impl IntoResponse {
        let namespace = {
            let s = state.read().await;
            query.namespace.clone().unwrap_or_else(|| s.namespace.clone())
        };

        match crate::snapshots::SnapshotManager::new(&namespace).await {
            Ok(manager) => match manager.list_all_snapshots().await {
                Ok(snapshots) => {
                    let items: Vec<SnapshotItem> = snapshots
                        .into_iter()
                        .map(|s| {
                            let age = s.age();
                            let status = s.status.to_string();
                            SnapshotItem {
                                name: s.name,
                                vm_name: s.vm_name,
                                status,
                                ready: s.ready_to_use,
                                age,
                            }
                        })
                        .collect();
                    let ctx = req_ctx(HttpMethod::GET, "/api/v1/snapshots");
                    ok_json(&ApiResponse::success(&items, &ctx.request_id))
                }
                Err(e) => err_json(500, "INTERNAL_ERROR", &sanitize_error(&e)),
            },
            Err(e) => err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        }
    }

    async fn list_vm_snapshots_handler(
        Path((ns, vm)): Path<(String, String)>,
    ) -> impl IntoResponse {
        if let Some(resp) = validate_k8s_params(&[("namespace", &ns), ("vm", &vm)]) {
            return resp;
        }

        match crate::snapshots::SnapshotManager::new(&ns).await {
            Ok(manager) => match manager.list_snapshots_for_vm(&vm).await {
                Ok(snapshots) => {
                    let items: Vec<SnapshotItem> = snapshots
                        .into_iter()
                        .map(|s| {
                            let age = s.age();
                            let status = s.status.to_string();
                            SnapshotItem {
                                name: s.name,
                                vm_name: s.vm_name,
                                status,
                                ready: s.ready_to_use,
                                age,
                            }
                        })
                        .collect();
                    let ctx = req_ctx(HttpMethod::GET, "/api/v1/snapshots/:ns/:vm");
                    ok_json(&ApiResponse::success(&items, &ctx.request_id))
                }
                Err(e) => err_json(500, "INTERNAL_ERROR", &sanitize_error(&e)),
            },
            Err(e) => err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        }
    }

    async fn delete_snapshot_handler(
        Path((ns, name)): Path<(String, String)>,
    ) -> impl IntoResponse {
        if let Some(resp) = validate_k8s_params(&[("namespace", &ns), ("name", &name)]) {
            return resp;
        }

        match crate::snapshots::SnapshotManager::new(&ns).await {
            Ok(manager) => match manager.delete_snapshot(&name).await {
                Ok(_) => {
                    let ctx = req_ctx(HttpMethod::POST, "/api/v1/snapshots/:ns/:name/delete");
                    ok_json(&ApiResponse::success(
                        &serde_json::json!({"message": format!("Snapshot '{}' deleted", name)}),
                        &ctx.request_id,
                    ))
                }
                Err(e) => err_json(500, "DELETE_FAILED", &sanitize_error(&e)),
            },
            Err(e) => err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        }
    }

    // ── Events ────────────────────────────────────────────────────

    #[derive(Deserialize)]
    pub struct EventsQuery {
        pub limit: Option<u32>,
    }

    async fn list_events_handler(
        State(state): State<SharedState>,
        Query(query): Query<EventsQuery>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();

        let limit = query.limit.unwrap_or(50).min(1000);

        use k8s_openapi::api::core::v1::Event;
        use kube::Api;
        let events_api: Api<Event> = Api::namespaced(client.client(), &namespace);
        let lp = kube::api::ListParams::default().limit(limit);
        match events_api.list(&lp).await {
            Ok(event_list) => {
                let items: Vec<EventItem> = event_list
                    .items
                    .into_iter()
                    .map(|e| EventItem {
                        type_: e.type_.unwrap_or_default(),
                        reason: e.reason.unwrap_or_default(),
                        message: e.message.unwrap_or_default(),
                        namespace: e.metadata.namespace.unwrap_or_default(),
                        involved_object: e.involved_object.name.unwrap_or_default(),
                        timestamp: e
                            .last_timestamp
                            .map(|t| t.0.to_rfc3339())
                            .or_else(|| {
                                e.metadata.creation_timestamp.map(|t| t.0.to_rfc3339())
                            })
                            .unwrap_or_default(),
                    })
                    .collect();
                let ctx = req_ctx(HttpMethod::GET, "/api/v1/events");
                ok_json(&ApiResponse::success(&items, &ctx.request_id))
            }
            Err(e) => err_json(500, "INTERNAL_ERROR", &sanitize_error(&e)),
        }
    }

    async fn recent_events_handler(
        State(state): State<SharedState>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();

        use k8s_openapi::api::core::v1::Event;
        use kube::Api;
        let events_api: Api<Event> = Api::namespaced(client.client(), &namespace);
        let lp = kube::api::ListParams::default();
        match events_api.list(&lp).await {
            Ok(event_list) => {
                let one_hour_ago = chrono::Utc::now() - chrono::TimeDelta::hours(1);
                let items: Vec<EventItem> = event_list
                    .items
                    .into_iter()
                    .filter(|e| {
                        // Keep events from the last hour based on timestamp
                        let ts = e
                            .last_timestamp
                            .as_ref()
                            .map(|t| t.0)
                            .or_else(|| e.metadata.creation_timestamp.as_ref().map(|t| t.0));
                        match ts {
                            Some(t) => t >= one_hour_ago,
                            None => false, // exclude events with no timestamp
                        }
                    })
                    .map(|e| EventItem {
                        type_: e.type_.unwrap_or_default(),
                        reason: e.reason.unwrap_or_default(),
                        message: e.message.unwrap_or_default(),
                        namespace: e.metadata.namespace.unwrap_or_default(),
                        involved_object: e.involved_object.name.unwrap_or_default(),
                        timestamp: e
                            .last_timestamp
                            .map(|t| t.0.to_rfc3339())
                            .or_else(|| {
                                e.metadata.creation_timestamp.map(|t| t.0.to_rfc3339())
                            })
                            .unwrap_or_default(),
                    })
                    .collect();
                let ctx = req_ctx(HttpMethod::GET, "/api/v1/events/recent");
                ok_json(&ApiResponse::success(&items, &ctx.request_id))
            }
            Err(e) => err_json(500, "INTERNAL_ERROR", &sanitize_error(&e)),
        }
    }

    // ── Dashboard Overview ────────────────────────────────────────

    async fn dashboard_overview_handler(
        State(state): State<SharedState>,
    ) -> impl IntoResponse {
        let s = state.read().await;
        let namespace = s.namespace.clone();
        let client = s.client();

        let vms = match client.list_vms(&namespace).await {
            Ok(vms) => vms,
            Err(e) => {
                return err_json(500, "INTERNAL_ERROR", &sanitize_error(&e));
            }
        };
        let total = vms.len();
        let mut running = 0usize;
        let mut stopped = 0usize;
        for vm in &vms {
            match vm
                .status
                .as_ref()
                .and_then(|s| s.printable_status.as_deref())
            {
                Some("Running") => running += 1,
                Some("Stopped") => stopped += 1,
                _ => {}
            }
        }
        let error = total.saturating_sub(running + stopped);

        // Count total allocated CPU/memory
        let mut total_cpus = 0u32;
        let mut total_memory_bytes = 0u64;
        for vm in &vms {
            total_cpus += vm
                .spec
                .template
                .spec
                .domain
                .cpu
                .as_ref()
                .and_then(|c| c.cores)
                .unwrap_or(1);
            if let Some(mem_str) = vm
                .spec
                .template
                .spec
                .domain
                .resources
                .requests
                .as_ref()
                .and_then(|r| r.get("memory"))
            {
                total_memory_bytes += parse_memory(mem_str);
            }
        }

        let snapshot_count = match crate::snapshots::SnapshotManager::new(&namespace).await {
            Ok(m) => m.list_all_snapshots().await.map(|s| s.len()).unwrap_or(0),
            Err(_) => 0,
        };

        let overview = DashboardOverview {
            cluster: ClusterStats {
                total_vms: total,
                running_vms: running,
                stopped_vms: stopped,
                error_vms: error,
                total_vcpus_allocated: total_cpus,
                total_memory_allocated_gb: (total_memory_bytes as f64)
                    / (1024.0 * 1024.0 * 1024.0),
                total_snapshots: snapshot_count,
            },
        };

        let ctx = req_ctx(HttpMethod::GET, "/api/v1/dashboard/overview");
        ok_json(&ApiResponse::success(&overview, &ctx.request_id))
    }

    // ── Health ─────────────────────────────────────────────────────

    async fn health_handler() -> impl IntoResponse {
        let ctx = req_ctx(HttpMethod::GET, "/api/v1/health");
        let health = serde_json::json!({
            "status": "healthy",
            "version": "v1",
            "service": "zorvia-api"
        });
        ok_json(&ApiResponse::success(&health, &ctx.request_id))
    }

    // ── Types ──────────────────────────────────────────────────────

    #[derive(Serialize)]
    struct VmDetail {
        #[serde(flatten)]
        info: VmInfo,
        vmi_status: Option<crate::kube::types::VirtualMachineInstanceStatus>,
    }

    #[derive(Serialize)]
    struct SnapshotItem {
        name: String,
        vm_name: String,
        status: String,
        ready: bool,
        age: String,
    }

    #[derive(Serialize)]
    struct EventItem {
        #[serde(rename = "type")]
        type_: String,
        reason: String,
        message: String,
        namespace: String,
        involved_object: String,
        timestamp: String,
    }

    #[derive(Serialize)]
    struct DashboardOverview {
        cluster: ClusterStats,
    }

    #[derive(Serialize)]
    struct ClusterStats {
        total_vms: usize,
        running_vms: usize,
        stopped_vms: usize,
        error_vms: usize,
        total_vcpus_allocated: u32,
        total_memory_allocated_gb: f64,
        total_snapshots: usize,
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn ok_json<T: Serialize>(data: &T) -> (StatusCode, Json<serde_json::Value>) {
        let value = serde_json::to_value(data).unwrap_or_else(|e| {
            serde_json::json!({"error": format!("serialization failed: {}", e)})
        });
        // Extract the status code from the serialized response if present,
        // so that 201/204/etc. responses get the correct HTTP status.
        let status_code = value
            .get("status")
            .and_then(|v| v.as_u64())
            .and_then(|s| StatusCode::from_u16(s as u16).ok())
            .unwrap_or(StatusCode::OK);
        (status_code, Json(value))
    }

    pub(crate) fn err_json(
        status: u16,
        code: &str,
        message: &str,
    ) -> (StatusCode, Json<serde_json::Value>) {
        let ctx = req_ctx(HttpMethod::GET, "");
        let resp = ApiResponse::error(status, code, message, &ctx.request_id);
        let value = serde_json::to_value(&resp).unwrap_or_else(|_| {
            serde_json::json!({"status": status, "error": code, "message": "internal error"})
        });
        (
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(value),
        )
    }

    fn parse_memory(s: &str) -> u64 {
        let s = s.trim();
        if let Some(val) = s.strip_suffix("Gi") {
            val.parse::<u64>().unwrap_or(0) * 1024 * 1024 * 1024
        } else if let Some(val) = s.strip_suffix("Mi") {
            val.parse::<u64>().unwrap_or(0) * 1024 * 1024
        } else if let Some(val) = s.strip_suffix("Ki") {
            val.parse::<u64>().unwrap_or(0) * 1024
        } else {
            s.parse::<u64>().unwrap_or(0)
        }
    }
}
