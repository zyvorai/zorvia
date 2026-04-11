// HTTP Server - Real web server using axum for serving API and dashboard
//
// This module is only compiled when the "web" feature is enabled.

#[cfg(feature = "web")]
pub mod web {
    use crate::api::{ApiResponse, HttpMethod, RequestContext};
    use crate::kube::KubeClient;
    use crate::tui::state::VmInfo;
    use axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{Html, IntoResponse, Json},
        routing::{delete, get, post},
        Router,
    };
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::sync::Mutex;

    pub struct WebState {
        pub namespace: String,
        pub kube_client: Option<KubeClient>,
    }

    impl WebState {
        pub fn new(namespace: String) -> Self {
            Self { namespace, kube_client: None }
        }

        pub async fn get_client(&mut self) -> anyhow::Result<&KubeClient> {
            if self.kube_client.is_none() {
                self.kube_client = Some(KubeClient::new().await?);
            }
            self.kube_client
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Kubernetes client not initialized"))
        }
    }

    pub type SharedState = Arc<Mutex<WebState>>;

    pub fn build_router(state: SharedState) -> Router {
        Router::new()
            // Dashboard
            .route("/dashboard", get(dashboard_handler))
            // VM endpoints
            .route("/api/v1/vms", get(list_vms_handler))
            .route("/api/v1/vms/:ns/:name", get(get_vm_handler))
            .route("/api/v1/vms/:ns/:name", delete(delete_vm_handler))
            .route("/api/v1/vms/:ns/:name/start", post(start_vm_handler))
            .route("/api/v1/vms/:ns/:name/stop", post(stop_vm_handler))
            .route("/api/v1/vms/:ns/:name/restart", post(restart_vm_handler))
            // Snapshots
            .route("/api/v1/snapshots", get(list_snapshots_handler))
            .route("/api/v1/snapshots/:ns/:vm", get(list_vm_snapshots_handler))
            .route("/api/v1/snapshots/:ns/:name/delete", post(delete_snapshot_handler))
            // Events
            .route("/api/v1/events", get(list_events_handler))
            .route("/api/v1/events/recent", get(recent_events_handler))
            // Dashboard overview
            .route("/api/v1/dashboard/overview", get(dashboard_overview_handler))
            // Health
            .route("/api/v1/health", get(health_handler))
            .with_state(state)
    }

    pub async fn start_server(host: &str, port: u16, namespace: String) -> anyhow::Result<()> {
        let state = Arc::new(Mutex::new(WebState::new(namespace)));
        let app = build_router(state);
        let addr = format!("{}:{}", host, port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn dashboard_handler() -> Html<&'static str> {
        Html(include_str!("web/dashboard.html"))
    }

    fn req_ctx(method: HttpMethod, path: &str) -> RequestContext {
        RequestContext::new(method, path)
    }

    /// Sanitize internal error details before sending to clients
    fn sanitize_error(e: &dyn std::fmt::Display) -> String {
        let msg = e.to_string();
        // Strip internal details — only keep the high-level message
        if let Some(pos) = msg.find(": ") {
            msg[..pos].to_string()
        } else {
            msg
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
        let mut s = state.lock().await;
        let namespace = query.namespace.clone().unwrap_or_else(|| s.namespace.clone());

        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

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
        let mut s = state.lock().await;
        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

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
            Err(_) => err_json(404, "NOT_FOUND", &format!("VM '{}' not found", name)),
        }
    }

    async fn start_vm_handler(
        State(state): State<SharedState>,
        Path((ns, name)): Path<(String, String)>,
    ) -> impl IntoResponse {
        let mut s = state.lock().await;
        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

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
        let mut s = state.lock().await;
        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

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
        let mut s = state.lock().await;
        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

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
        let mut s = state.lock().await;
        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

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
            let s = state.lock().await;
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

    async fn list_events_handler(
        State(state): State<SharedState>,
    ) -> impl IntoResponse {
        let mut s = state.lock().await;
        let namespace = s.namespace.clone();

        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

        use k8s_openapi::api::core::v1::Event;
        use kube::Api;
        let events_api: Api<Event> = Api::namespaced(client.client(), &namespace);
        let lp = kube::api::ListParams::default().limit(50);
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
        // Delegate to full events handler with implicit limit
        list_events_handler(State(state)).await
    }

    // ── Dashboard Overview ────────────────────────────────────────

    async fn dashboard_overview_handler(
        State(state): State<SharedState>,
    ) -> impl IntoResponse {
        let mut s = state.lock().await;
        let namespace = s.namespace.clone();

        let client = match s.get_client().await {
            Ok(c) => c,
            Err(e) => return err_json(503, "SERVICE_UNAVAILABLE", &sanitize_error(&e)),
        };

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
            match vm.status.as_ref().and_then(|s| s.print_able_status.as_deref()) {
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
        (StatusCode::OK, Json(value))
    }

    fn err_json(
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
