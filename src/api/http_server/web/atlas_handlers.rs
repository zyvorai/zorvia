// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use super::{SharedAuditTrail, SharedState};
use crate::atlas::models::CreateVolumeRequest;
use crate::atlas::{Client, Error as AtlasError};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

fn error_response(error: AtlasError) -> Response {
    match error {
        AtlasError::Upstream {
            status,
            code,
            message,
        } => {
            let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            (
                status,
                Json(serde_json::json!({
                    "success": false,
                    "error": {
                        "code": code.unwrap_or_else(|| "ATLAS_UPSTREAM".into()),
                        "message": message,
                    }
                })),
            )
                .into_response()
        }
        AtlasError::MissingTenant => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": {"code": "ATLAS_TENANT_REQUIRED", "message": error.to_string()}
            })),
        )
            .into_response(),
        AtlasError::Transport(_) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "success": false,
                "error": {
                    "code": "ATLAS_UNREACHABLE",
                    "message": "Zorvia could not reach the configured Atlas control plane"
                }
            })),
        )
            .into_response(),
    }
}

async fn client(state: &SharedState) -> Result<Client, Box<Response>> {
    state.read().await.atlas.clone().ok_or_else(|| {
        Box::new(
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "success": false,
                    "error": {
                        "code": "ATLAS_DISABLED",
                        "message": "Atlas integration is disabled; configure ATLAS_URL on the Zorvia server"
                    }
                })),
            )
                .into_response(),
        )
    })
}

fn caller(auth: &Option<axum::Extension<crate::api::auth::AuthIdentity>>) -> String {
    auth.as_ref()
        .and_then(|a| a.0.username.clone())
        .unwrap_or_else(|| "api-token".into())
}

async fn audit_atlas_volume(
    audit: &SharedAuditTrail,
    user: &str,
    action: crate::audit_trail::AuditAction,
    name: &str,
    success: bool,
    details: serde_json::Value,
) {
    let entry = crate::audit_trail::AuditEntry {
        id: crate::utils::generate_id("audit", name),
        timestamp: chrono::Utc::now(),
        user: user.to_string(),
        severity: crate::audit_trail::AuditSeverity::Info,
        action,
        resource_type: "atlas_volume".into(),
        resource_name: name.to_string(),
        namespace: String::new(),
        details,
        ip_address: String::new(),
        success,
    };
    audit.write().await.record(entry);
}

#[derive(Debug, Deserialize)]
pub(super) struct ConfirmQuery {
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ExpandVolumeBody {
    new_size_bytes: i64,
}

pub(super) async fn atlas_status(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(c) => c,
        Err(_) => {
            return Json(serde_json::json!({
                "enabled": false,
                "connected": false,
                "tenant_id": null,
            }))
            .into_response()
        }
    };
    match c.health().await {
        Ok(health) => Json(serde_json::json!({
            "enabled": true,
            "connected": true,
            "tenant_id": c.configured_tenant_id(),
            "health": health,
        }))
        .into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "enabled": true,
                "connected": false,
                "tenant_id": c.configured_tenant_id(),
                "error": error.to_string(),
            })),
        )
            .into_response(),
    }
}

pub(super) async fn atlas_list_backends(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.list_backends().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_backends_summary(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.backends_summary().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_list_clusters(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.list_clusters().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_cluster_health(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.cluster_health(&id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_list_pools(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.list_pools().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_ceph_status(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.ceph_status().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_ceph_df(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.ceph_df().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_list_storage_classes(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.list_storage_classes().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_list_volumes(State(state): State<SharedState>) -> Response {
    let c = match client(&state).await {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match c.list_volumes().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error_response(e),
    }
}

fn disabled_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "success": false,
            "error": {
                "code": "ATLAS_DISABLED",
                "message": "Atlas integration is disabled; configure ATLAS_URL on the Zorvia server"
            }
        })),
    )
        .into_response()
}

pub(super) async fn atlas_create_volume(
    State(state): State<SharedState>,
    auth: Option<axum::Extension<crate::api::auth::AuthIdentity>>,
    Json(body): Json<CreateVolumeRequest>,
) -> Response {
    let s = state.read().await;
    let Some(c) = s.atlas.clone() else {
        return disabled_response();
    };
    let audit = s.audit.clone();
    drop(s);

    let name = body.name.clone();
    let result = c.create_volume(body).await;
    audit_atlas_volume(
        &audit,
        &caller(&auth),
        crate::audit_trail::AuditAction::Create,
        &name,
        result.is_ok(),
        match &result {
            Ok(v) => v.clone(),
            Err(e) => serde_json::json!({ "error": e.to_string() }),
        },
    )
    .await;
    match result {
        Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_expand_volume(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    auth: Option<axum::Extension<crate::api::auth::AuthIdentity>>,
    Json(body): Json<ExpandVolumeBody>,
) -> Response {
    let s = state.read().await;
    let Some(c) = s.atlas.clone() else {
        return disabled_response();
    };
    let audit = s.audit.clone();
    drop(s);

    let result = c.expand_volume(&id, body.new_size_bytes).await;
    audit_atlas_volume(
        &audit,
        &caller(&auth),
        crate::audit_trail::AuditAction::ScaleUp,
        &id,
        result.is_ok(),
        match &result {
            Ok(v) => v.clone(),
            Err(e) => {
                serde_json::json!({ "new_size_bytes": body.new_size_bytes, "error": e.to_string() })
            }
        },
    )
    .await;
    match result {
        Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
        Err(e) => error_response(e),
    }
}

pub(super) async fn atlas_delete_volume(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(query): Query<ConfirmQuery>,
    auth: Option<axum::Extension<crate::api::auth::AuthIdentity>>,
) -> Response {
    let s = state.read().await;
    let Some(c) = s.atlas.clone() else {
        return disabled_response();
    };
    let audit = s.audit.clone();
    drop(s);

    let result = c.delete_volume(&id, query.confirm).await;
    audit_atlas_volume(
        &audit,
        &caller(&auth),
        crate::audit_trail::AuditAction::Delete,
        &id,
        result.is_ok(),
        match &result {
            Ok(v) => v.clone(),
            Err(e) => serde_json::json!({ "error": e.to_string() }),
        },
    )
    .await;
    match result {
        Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
        Err(e) => error_response(e),
    }
}
