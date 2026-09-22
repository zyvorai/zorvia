//! Audit Trail - Comprehensive audit event tracking with optional SQLite persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "web")]
use std::path::Path;
#[cfg(feature = "web")]
use std::sync::Mutex;

use crate::utils::generate_id;

#[derive(Debug)]
pub struct AuditTrail {
    pub entries: Vec<AuditEntry>,
    pub max_entries: usize,
    /// When set, every `record` is also written to SQLite (web builds only).
    #[cfg(feature = "web")]
    db: Option<Mutex<rusqlite::Connection>>,
}

impl Clone for AuditTrail {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            max_entries: self.max_entries,
            #[cfg(feature = "web")]
            db: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub action: AuditAction,
    pub resource_type: String,
    pub resource_name: String,
    pub namespace: String,
    pub details: Value,
    pub ip_address: String,
    pub success: bool,
    pub severity: AuditSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditAction {
    Create,
    Update,
    Delete,
    Start,
    Stop,
    Restart,
    Migrate,
    Clone,
    Snapshot,
    Restore,
    ConfigChange,
    Login,
    Logout,
    Export,
    Import,
    ScaleUp,
    ScaleDown,
    Approve,
    Reject,
    ScheduleChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum AuditSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl AuditTrail {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            #[cfg(feature = "web")]
            db: None,
        }
    }

    /// Prefer `ZORVIA_AUDIT_DB`, else alongside auth DB as `audit.db`, else memory-only.
    pub fn from_env() -> Self {
        #[cfg(feature = "web")]
        {
            let path = std::env::var("ZORVIA_AUDIT_DB").ok().or_else(|| {
                std::env::var("ZORVIA_AUTH_DB").ok().map(|auth| {
                    let p = std::path::PathBuf::from(auth);
                    p.parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .join("audit.db")
                        .to_string_lossy()
                        .to_string()
                })
            });
            if let Some(p) = path {
                return match Self::open_persistent(&p, 10000) {
                    Ok(t) => t,
                    Err(e) => {
                        log::warn!("Audit persistence unavailable ({e}); using in-memory trail");
                        Self::default()
                    }
                };
            }
        }
        Self::default()
    }

    #[cfg(feature = "web")]
    pub fn open_persistent(path: impl AsRef<Path>, max_entries: usize) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_entries (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                user_name TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_name TEXT NOT NULL,
                namespace TEXT NOT NULL,
                details TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                success INTEGER NOT NULL,
                severity TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit_entries(timestamp);",
        )?;
        let mut trail = Self {
            entries: Vec::new(),
            max_entries,
            db: Some(Mutex::new(conn)),
        };
        trail.reload_from_db()?;
        log::info!(
            "Audit trail: persistent store at {} ({} entries loaded)",
            path.display(),
            trail.entries.len()
        );
        Ok(trail)
    }

    #[cfg(feature = "web")]
    fn reload_from_db(&mut self) -> anyhow::Result<()> {
        let Some(db) = &self.db else {
            return Ok(());
        };
        let conn = db.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, user_name, action, resource_type, resource_name, namespace,
                    details, ip_address, success, severity
             FROM audit_entries ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([self.max_entries as i64], |row| {
            let details_s: String = row.get(7)?;
            let success: i64 = row.get(9)?;
            Ok(AuditEntry {
                id: row.get(0)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                user: row.get(2)?,
                action: action_from_str(&row.get::<_, String>(3)?),
                resource_type: row.get(4)?,
                resource_name: row.get(5)?,
                namespace: row.get(6)?,
                details: serde_json::from_str(&details_s).unwrap_or(Value::Null),
                ip_address: row.get(8)?,
                success: success != 0,
                severity: severity_from_str(&row.get::<_, String>(10)?),
            })
        })?;
        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        entries.reverse();
        self.entries = entries;
        Ok(())
    }

    pub fn record(&mut self, entry: AuditEntry) {
        #[cfg(feature = "web")]
        if let Some(db) = &self.db {
            if let Ok(conn) = db.lock() {
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO audit_entries
                     (id, timestamp, user_name, action, resource_type, resource_name, namespace,
                      details, ip_address, success, severity)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    rusqlite::params![
                        entry.id,
                        entry.timestamp.to_rfc3339(),
                        entry.user,
                        action_to_str(&entry.action),
                        entry.resource_type,
                        entry.resource_name,
                        entry.namespace,
                        entry.details.to_string(),
                        entry.ip_address,
                        if entry.success { 1 } else { 0 },
                        severity_to_str(&entry.severity),
                    ],
                );
            }
        }
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            let excess = self.entries.len().saturating_sub(self.max_entries);
            if excess > 0 {
                self.entries.drain(0..excess);
            }
        }
    }

    pub fn log_action(
        &mut self,
        user: &str,
        action: AuditAction,
        resource_type: &str,
        resource_name: &str,
        namespace: &str,
        success: bool,
    ) {
        let severity = match &action {
            AuditAction::Delete | AuditAction::Migrate => AuditSeverity::High,
            AuditAction::Create | AuditAction::Update | AuditAction::ConfigChange => {
                AuditSeverity::Medium
            }
            AuditAction::Start | AuditAction::Stop | AuditAction::Restart => AuditSeverity::Low,
            _ => AuditSeverity::Info,
        };
        self.record(AuditEntry {
            id: generate_id("audit", resource_name),
            timestamp: Utc::now(),
            user: user.to_string(),
            action,
            resource_type: resource_type.to_string(),
            resource_name: resource_name.to_string(),
            namespace: namespace.to_string(),
            details: Value::Null,
            ip_address: "127.0.0.1".to_string(),
            success,
            severity,
        });
    }

    pub fn query(
        &self,
        user: Option<&str>,
        action: Option<&AuditAction>,
        namespace: Option<&str>,
        min_severity: Option<&AuditSeverity>,
    ) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| {
                user.is_none_or( |u| e.user == u)
                    && action.is_none_or( |a| e.action == *a)
                    && namespace.is_none_or( |ns| e.namespace == ns)
                    && min_severity.is_none_or( |s| e.severity >= *s)
            })
            .collect()
    }

    pub fn recent(&self, limit: usize) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn by_resource(&self, resource_name: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.resource_name == resource_name)
            .collect()
    }

    pub fn failed_actions(&self) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| !e.success).collect()
    }

    pub fn stats(&self) -> AuditStats {
        AuditStats {
            total: self.entries.len(),
            successful: self.entries.iter().filter(|e| e.success).count(),
            failed: self.entries.iter().filter(|e| !e.success).count(),
            critical: self
                .entries
                .iter()
                .filter(|e| e.severity == AuditSeverity::Critical)
                .count(),
            high: self
                .entries
                .iter()
                .filter(|e| e.severity == AuditSeverity::High)
                .count(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditStats {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub critical: usize,
    pub high: usize,
}

impl Default for AuditTrail {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[cfg(feature = "web")]
fn action_to_str(a: &AuditAction) -> &'static str {
    match a {
        AuditAction::Create => "Create",
        AuditAction::Update => "Update",
        AuditAction::Delete => "Delete",
        AuditAction::Start => "Start",
        AuditAction::Stop => "Stop",
        AuditAction::Restart => "Restart",
        AuditAction::Migrate => "Migrate",
        AuditAction::Clone => "Clone",
        AuditAction::Snapshot => "Snapshot",
        AuditAction::Restore => "Restore",
        AuditAction::ConfigChange => "ConfigChange",
        AuditAction::Login => "Login",
        AuditAction::Logout => "Logout",
        AuditAction::Export => "Export",
        AuditAction::Import => "Import",
        AuditAction::ScaleUp => "ScaleUp",
        AuditAction::ScaleDown => "ScaleDown",
        AuditAction::Approve => "Approve",
        AuditAction::Reject => "Reject",
        AuditAction::ScheduleChange => "ScheduleChange",
    }
}

#[cfg(feature = "web")]
fn action_from_str(s: &str) -> AuditAction {
    match s {
        "Create" => AuditAction::Create,
        "Update" => AuditAction::Update,
        "Delete" => AuditAction::Delete,
        "Start" => AuditAction::Start,
        "Stop" => AuditAction::Stop,
        "Restart" => AuditAction::Restart,
        "Migrate" => AuditAction::Migrate,
        "Clone" => AuditAction::Clone,
        "Snapshot" => AuditAction::Snapshot,
        "Restore" => AuditAction::Restore,
        "ConfigChange" => AuditAction::ConfigChange,
        "Login" => AuditAction::Login,
        "Logout" => AuditAction::Logout,
        "Export" => AuditAction::Export,
        "Import" => AuditAction::Import,
        "ScaleUp" => AuditAction::ScaleUp,
        "ScaleDown" => AuditAction::ScaleDown,
        "Approve" => AuditAction::Approve,
        "Reject" => AuditAction::Reject,
        "ScheduleChange" => AuditAction::ScheduleChange,
        _ => AuditAction::Update,
    }
}

#[cfg(feature = "web")]
fn severity_to_str(s: &AuditSeverity) -> &'static str {
    match s {
        AuditSeverity::Info => "Info",
        AuditSeverity::Low => "Low",
        AuditSeverity::Medium => "Medium",
        AuditSeverity::High => "High",
        AuditSeverity::Critical => "Critical",
    }
}

#[cfg(feature = "web")]
fn severity_from_str(s: &str) -> AuditSeverity {
    match s {
        "Low" => AuditSeverity::Low,
        "Medium" => AuditSeverity::Medium,
        "High" => AuditSeverity::High,
        "Critical" => AuditSeverity::Critical,
        _ => AuditSeverity::Info,
    }
}

#[cfg(all(test, feature = "web"))]
mod tests {
    use super::*;

    #[test]
    fn persistent_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.db");
        {
            let mut trail = AuditTrail::open_persistent(&path, 100).unwrap();
            trail.log_action("admin", AuditAction::Login, "auth", "login", "ns", true);
            assert_eq!(trail.entries.len(), 1);
        }
        let trail2 = AuditTrail::open_persistent(&path, 100).unwrap();
        assert_eq!(trail2.entries.len(), 1);
        assert_eq!(trail2.entries[0].user, "admin");
        assert_eq!(trail2.entries[0].action, AuditAction::Login);
    }
}
