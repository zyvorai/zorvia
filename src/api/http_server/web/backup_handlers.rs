//! Real VM backup API backed by `crate::snapshots::SnapshotManager` -- the
//! same live VolumeSnapshot infrastructure `Snapshots.tsx` already uses,
//! reframed fleet-wide as "backups" instead of one VM at a time.
//!
//! Fixes a live bug: `web/src/api/backup.ts`'s `createBackup()` is already
//! called today by `VMList.tsx`'s bulk-backup action, but `/api/backups`
//! never existed anywhere in the router -- it silently 404'd.
//!
//! Every real snapshot is a complete point-in-time capture (there is no
//! incremental-backup engine here), so `backup_type` is accepted on create
//! but always reported back as `"full"`. `retention_days` is stored as a
//! label for future use; nothing currently enforces it automatically --
//! `SnapshotManager::apply_retention_policy` exists and works, it's just
//! not wired to a scheduler yet (that's `BackupScheduler`, a bigger lift
//! for its own slice).

use super::*;
use axum::extract::Json as AxumJson;
use crate::snapshots::{SnapshotConfig, SnapshotInfo, SnapshotManager, SnapshotStatus};
use serde_json::json;

const LABEL_RETENTION_DAYS: &str = "zorvia.io/retention-days";

fn backup_json(s: &SnapshotInfo) -> serde_json::Value {
    let status = match s.status {
        SnapshotStatus::Succeeded => "completed",
        SnapshotStatus::InProgress => "in_progress",
        SnapshotStatus::Failed => "failed",
        SnapshotStatus::Unknown => "in_progress",
    };
    let retention_days: Option<u32> = s
        .labels
        .get(LABEL_RETENTION_DAYS)
        .and_then(|v| v.parse().ok());
    let expires_at = match (s.created_at, retention_days) {
        (Some(created), Some(days)) => Some((created + chrono::Duration::days(days as i64)).to_rfc3339()),
        _ => None,
    };
    json!({
        "id": s.name,
        "vm_name": s.vm_name,
        "backup_type": "full",
        "size_bytes": s.size.as_deref().and_then(|sz| sz.trim_end_matches(|c: char| !c.is_ascii_digit()).parse::<u64>().ok()).unwrap_or(0),
        "compressed": false,
        "created": s.created_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
        "status": status,
        "storage_location": format!("kubevirt-snapshot:{}/{}", s.namespace, s.name),
        "retention_days": retention_days.unwrap_or(0),
        "expires_at": expires_at,
        "metadata": serde_json::Value::Null,
    })
}

fn backup_job_json(s: &SnapshotInfo) -> serde_json::Value {
    let status = match s.status {
        SnapshotStatus::Succeeded => "completed",
        SnapshotStatus::InProgress => "running",
        SnapshotStatus::Failed => "failed",
        SnapshotStatus::Unknown => "running",
    };
    json!({
        "id": s.name,
        "backup_id": s.name,
        "vm_name": s.vm_name,
        "operation": "backup",
        "status": status,
        "progress": if s.ready_to_use { 100 } else { 0 },
        "started_at": s.created_at.map(|t| t.to_rfc3339()),
        "completed_at": s.completed_at.map(|t| t.to_rfc3339()),
        "error": s.error,
    })
}

fn backup_error(action: &str, e: impl std::fmt::Display) -> axum::response::Response {
    let raw = sanitize_error(&e);
    let lower = raw.to_ascii_lowercase();
    let (code, kind) = if lower.contains("notfound") || lower.contains("not found") {
        (404, "NOT_FOUND")
    } else if lower.contains("already exists") || lower.contains("conflict") {
        (409, "CONFLICT")
    } else {
        (500, "BACKUP_FAILED")
    };
    let (st, j) = err_json(code, kind, &format!("Failed to {action}: {raw}"));
    (st, j).into_response()
}

#[derive(Debug, Deserialize)]
pub struct ListBackupsQuery {
    #[serde(default)]
    pub vm: Option<String>,
}

pub async fn list_backups_handler(
    State(state): State<SharedState>,
    Query(q): Query<ListBackupsQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    drop(s);

    let manager = match SnapshotManager::new(&namespace).await {
        Ok(m) => m,
        Err(e) => return backup_error("connect to cluster", e),
    };
    match manager.list_snapshots_sorted(q.vm.as_deref()).await {
        Ok(snaps) => Json(snaps.iter().map(backup_json).collect::<Vec<_>>()).into_response(),
        Err(e) => backup_error("list backups", e),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateBackupBody {
    pub vm_name: String,
    #[serde(default)]
    pub backup_type: Option<String>,
    #[serde(default)]
    pub retention_days: Option<u32>,
    #[serde(default)]
    pub description: Option<String>,
}

pub async fn create_backup_handler(
    State(state): State<SharedState>,
    AxumJson(body): AxumJson<CreateBackupBody>,
) -> impl IntoResponse {
    if body.vm_name.trim().is_empty() {
        let (st, j) = err_json(400, "INVALID_NAME", "vm_name is required");
        return (st, j).into_response();
    }
    let s = state.read().await;
    let namespace = s.namespace.clone();
    drop(s);

    let manager = match SnapshotManager::new(&namespace).await {
        Ok(m) => m,
        Err(e) => return backup_error("connect to cluster", e),
    };

    let backup_name = format!(
        "backup-{}-{}",
        body.vm_name,
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    );
    let mut config = SnapshotConfig::new(&body.vm_name, &backup_name)
        .with_label("zorvia.io/backup", "true")
        .with_label(
            "zorvia.io/backup-type",
            body.backup_type.as_deref().unwrap_or("full"),
        );
    if let Some(days) = body.retention_days {
        config = config.with_label(LABEL_RETENTION_DAYS, days.to_string());
    }
    if let Some(desc) = body.description {
        config = config.with_description(desc);
    }

    match manager.create_snapshot(&config).await {
        Ok(info) => (StatusCode::CREATED, Json(backup_job_json(&info))).into_response(),
        Err(e) => backup_error("create backup", e),
    }
}

pub async fn delete_backup_handler(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    drop(s);

    let manager = match SnapshotManager::new(&namespace).await {
        Ok(m) => m,
        Err(e) => return backup_error("connect to cluster", e),
    };
    match manager.delete_snapshot(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => backup_error("delete backup", e),
    }
}
