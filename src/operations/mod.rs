//! Durable long-running operations.
//!
//! Work such as image capture, backups and migrations used to live in
//! in-memory registries and lose all state on restart. `OperationsDb` persists
//! each operation (state, phase, progress, params, result) in SQLite so a
//! reconciler can resume it after a restart, honour cancellation, and stop
//! retrying after `max_attempts`.
//!
//! State machine:
//! `queued -> running -> succeeded | failed | cancelled`, with
//! `running -> queued` on a retryable failure or after a restart.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

pub mod runtime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl OpState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Queued,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub kind: String,
    pub resource: String,
    pub namespace: String,
    pub state: OpState,
    pub phase: String,
    pub progress: u8,
    pub params: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub attempts: u32,
    pub max_attempts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub cancel_requested: bool,
    pub created: String,
    pub updated: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
}

/// Input for `OperationsDb::create`.
#[derive(Debug, Clone)]
pub struct NewOperation {
    pub kind: String,
    pub resource: String,
    pub namespace: String,
    pub params: serde_json::Value,
    pub max_attempts: u32,
    /// A second `create` with the same key returns the existing operation
    /// instead of starting duplicate work.
    pub idempotency_key: Option<String>,
    pub owner: Option<String>,
}

impl NewOperation {
    pub fn new(kind: &str, resource: &str, namespace: &str) -> Self {
        Self {
            kind: kind.to_string(),
            resource: resource.to_string(),
            namespace: namespace.to_string(),
            params: serde_json::Value::Null,
            max_attempts: 3,
            idempotency_key: None,
            owner: None,
        }
    }
}

impl OperationsDb {
    /// Process-wide store. Falls back to an in-memory DB (with a warning)
    /// if the file cannot be opened, so the server still starts.
    pub fn global() -> &'static std::sync::Arc<OperationsDb> {
        static DB: std::sync::OnceLock<std::sync::Arc<OperationsDb>> = std::sync::OnceLock::new();
        DB.get_or_init(|| {
            std::sync::Arc::new(Self::from_env().unwrap_or_else(|e| {
                log::warn!("operations DB unavailable ({e}); using in-memory store");
                Self::open(":memory:").expect("in-memory sqlite")
            }))
        })
    }
}

/// Where the operations database lives. `ZORVIA_OPS_DB` wins; otherwise it sits
/// next to the auth database (like the audit trail), so a deployment that puts
/// `ZORVIA_AUTH_DB` on a persistent volume keeps operations across restarts;
/// otherwise the per-user data directory. (Without this, a container restart
/// silently discarded every operation.)
pub fn ops_db_path(get: impl Fn(&str) -> Option<String>) -> String {
    if let Some(p) = get("ZORVIA_OPS_DB").filter(|p| !p.trim().is_empty()) {
        return p;
    }
    if let Some(auth) = get("ZORVIA_AUTH_DB").filter(|p| !p.trim().is_empty()) {
        if let Some(dir) = std::path::Path::new(&auth).parent() {
            if dir != std::path::Path::new("") {
                return dir.join("ops.db").to_string_lossy().to_string();
            }
        }
    }
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zorvia")
        .join("ops.db")
        .to_string_lossy()
        .to_string()
}

pub struct OperationsDb {
    conn: Mutex<rusqlite::Connection>,
}

const COLS: &str = "id, kind, resource, namespace, state, phase, progress, params, result, \
                    error, attempts, max_attempts, idempotency_key, owner, cancel_requested, \
                    created, updated, completed";

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn row_to_op(r: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    let params_s: String = r.get(7)?;
    let result_s: Option<String> = r.get(8)?;
    Ok(Operation {
        id: r.get(0)?,
        kind: r.get(1)?,
        resource: r.get(2)?,
        namespace: r.get(3)?,
        state: OpState::parse(&r.get::<_, String>(4)?),
        phase: r.get(5)?,
        progress: r.get::<_, i64>(6)?.clamp(0, 100) as u8,
        params: serde_json::from_str(&params_s).unwrap_or(serde_json::Value::Null),
        result: result_s.and_then(|s| serde_json::from_str(&s).ok()),
        error: r.get(9)?,
        attempts: r.get::<_, i64>(10)?.max(0) as u32,
        max_attempts: r.get::<_, i64>(11)?.max(1) as u32,
        idempotency_key: r.get(12)?,
        owner: r.get(13)?,
        cancel_requested: r.get::<_, i64>(14)? != 0,
        created: r.get(15)?,
        updated: r.get(16)?,
        completed: r.get(17)?,
    })
}

impl OperationsDb {
    pub fn open(path: &str) -> Result<Self> {
        let conn = if path == ":memory:" {
            rusqlite::Connection::open_in_memory()?
        } else {
            if let Some(parent) = std::path::Path::new(path).parent() {
                std::fs::create_dir_all(parent).ok();
            }
            rusqlite::Connection::open(path)?
        };
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS operations (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                resource TEXT NOT NULL,
                namespace TEXT NOT NULL,
                state TEXT NOT NULL,
                phase TEXT NOT NULL DEFAULT '',
                progress INTEGER NOT NULL DEFAULT 0,
                params TEXT NOT NULL DEFAULT 'null',
                result TEXT,
                error TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 3,
                idempotency_key TEXT,
                owner TEXT,
                cancel_requested INTEGER NOT NULL DEFAULT 0,
                created TEXT NOT NULL,
                updated TEXT NOT NULL,
                completed TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_operations_idem
                ON operations(idempotency_key) WHERE idempotency_key IS NOT NULL;
            CREATE INDEX IF NOT EXISTS idx_operations_state ON operations(state);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn from_env() -> Result<Self> {
        Self::open(&ops_db_path(|k| std::env::var(k).ok()))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Create an operation. Returns `(operation, created)`; `created` is
    /// false when `idempotency_key` matched an existing operation.
    pub fn create(&self, new: NewOperation) -> Result<(Operation, bool)> {
        let conn = self.lock();
        if let Some(key) = &new.idempotency_key {
            let existing = conn
                .query_row(
                    &format!("SELECT {COLS} FROM operations WHERE idempotency_key = ?1"),
                    params![key],
                    row_to_op,
                )
                .optional()?;
            if let Some(op) = existing {
                return Ok((op, false));
            }
        }
        let id = format!("op-{}", uuid::Uuid::new_v4().simple());
        let ts = now();
        conn.execute(
            "INSERT INTO operations (id, kind, resource, namespace, state, params, \
             max_attempts, idempotency_key, owner, created, updated) \
             VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id,
                new.kind,
                new.resource,
                new.namespace,
                new.params.to_string(),
                new.max_attempts.max(1),
                new.idempotency_key,
                new.owner,
                ts
            ],
        )
        .context("insert operation")?;
        let op = conn.query_row(
            &format!("SELECT {COLS} FROM operations WHERE id = ?1"),
            params![id],
            row_to_op,
        )?;
        Ok((op, true))
    }

    pub fn get(&self, id: &str) -> Result<Option<Operation>> {
        Ok(self
            .lock()
            .query_row(
                &format!("SELECT {COLS} FROM operations WHERE id = ?1"),
                params![id],
                row_to_op,
            )
            .optional()?)
    }

    /// Newest first, optionally filtered by kind.
    pub fn list(&self, kind: Option<&str>, limit: usize) -> Result<Vec<Operation>> {
        let conn = self.lock();
        let limit = limit.clamp(1, 500) as i64;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM operations WHERE (?1 IS NULL OR kind = ?1) \
             ORDER BY created DESC, rowid DESC LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![kind, limit], row_to_op)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Operations the reconciler should still act on, oldest first.
    pub fn list_active(&self) -> Result<Vec<Operation>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM operations WHERE state IN ('queued','running') \
             ORDER BY created ASC, rowid ASC"
        ))?;
        let rows = stmt.query_map([], row_to_op)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// queued -> running, counting the attempt. Returns false if the
    /// operation was not queued (already claimed, cancelled, or finished).
    pub fn start(&self, id: &str) -> Result<bool> {
        let n = self.lock().execute(
            "UPDATE operations SET state='running', attempts=attempts+1, updated=?2 \
             WHERE id=?1 AND state='queued'",
            params![id, now()],
        )?;
        Ok(n == 1)
    }

    /// Record phase/progress of a running operation (progress is capped at
    /// 99 -- 100 is only set by `succeed`).
    pub fn set_progress(&self, id: &str, phase: &str, progress: u8) -> Result<()> {
        self.lock().execute(
            "UPDATE operations SET phase=?2, progress=?3, updated=?4 \
             WHERE id=?1 AND state='running'",
            params![id, phase, progress.min(99), now()],
        )?;
        Ok(())
    }

    pub fn succeed(&self, id: &str, result: Option<&serde_json::Value>) -> Result<()> {
        let ts = now();
        self.lock().execute(
            "UPDATE operations SET state='succeeded', progress=100, result=?2, error=NULL, \
             updated=?3, completed=?3 WHERE id=?1 AND state='running'",
            params![id, result.map(|r| r.to_string()), ts],
        )?;
        Ok(())
    }

    /// Record a failure. When `retryable` and attempts remain the operation
    /// goes back to `queued`; otherwise it becomes `failed`. Returns the
    /// resulting state.
    pub fn fail(&self, id: &str, error: &str, retryable: bool) -> Result<OpState> {
        let conn = self.lock();
        let (attempts, max, cancel): (i64, i64, i64) = conn
            .query_row(
                "SELECT attempts, max_attempts, cancel_requested FROM operations WHERE id=?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| anyhow!("operation {id} not found"))?;
        let ts = now();
        if retryable && cancel == 0 && attempts < max {
            conn.execute(
                "UPDATE operations SET state='queued', error=?2, updated=?3 \
                 WHERE id=?1 AND state='running'",
                params![id, error, ts],
            )?;
            Ok(OpState::Queued)
        } else {
            conn.execute(
                "UPDATE operations SET state='failed', error=?2, updated=?3, completed=?3 \
                 WHERE id=?1 AND state IN ('running','queued')",
                params![id, error, ts],
            )?;
            Ok(OpState::Failed)
        }
    }

    /// Ask for cancellation. A queued operation is cancelled immediately; a
    /// running one is flagged so its handler can stop and call
    /// `mark_cancelled`. Returns the resulting state, or `None` if unknown.
    pub fn request_cancel(&self, id: &str) -> Result<Option<OpState>> {
        let conn = self.lock();
        let ts = now();
        conn.execute(
            "UPDATE operations SET state='cancelled', cancel_requested=1, updated=?2, \
             completed=?2 WHERE id=?1 AND state='queued'",
            params![id, ts],
        )?;
        conn.execute(
            "UPDATE operations SET cancel_requested=1, updated=?2 \
             WHERE id=?1 AND state='running'",
            params![id, ts],
        )?;
        let state: Option<String> = conn
            .query_row(
                "SELECT state FROM operations WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(state.map(|s| OpState::parse(&s)))
    }

    pub fn mark_cancelled(&self, id: &str) -> Result<()> {
        let ts = now();
        self.lock().execute(
            "UPDATE operations SET state='cancelled', updated=?2, completed=?2 \
             WHERE id=?1 AND state IN ('running','queued')",
            params![id, ts],
        )?;
        Ok(())
    }

    /// Heartbeat from the process running an operation, so other replicas
    /// can tell it apart from one orphaned by a dead leader.
    pub fn touch(&self, id: &str) -> Result<()> {
        self.lock().execute(
            "UPDATE operations SET updated=?2 WHERE id=?1 AND state='running'",
            params![id, now()],
        )?;
        Ok(())
    }

    /// running -> queued for an operation whose owner stopped heartbeating.
    /// Does not touch `attempts` (the interrupted attempt already counted);
    /// fails it instead if that was its last attempt.
    pub fn requeue_orphan(&self, id: &str) -> Result<OpState> {
        let conn = self.lock();
        let ts = now();
        conn.execute(
            "UPDATE operations SET state='failed', updated=?2, completed=?2, \
             error='owner stopped responding; retry limit reached' \
             WHERE id=?1 AND state='running' AND attempts >= max_attempts",
            params![id, ts],
        )?;
        conn.execute(
            "UPDATE operations SET state='queued', updated=?2 \
             WHERE id=?1 AND state='running'",
            params![id, ts],
        )?;
        let state: String = conn.query_row(
            "SELECT state FROM operations WHERE id=?1",
            params![id],
            |r| r.get(0),
        )?;
        Ok(OpState::parse(&state))
    }

    /// Startup recovery: anything left `running` by a previous process is
    /// re-queued (its attempt already counted), or failed if it has used all
    /// its attempts or was being cancelled. Returns how many were touched.
    pub fn recover_interrupted(&self) -> Result<usize> {
        let conn = self.lock();
        let ts = now();
        let cancelled = conn.execute(
            "UPDATE operations SET state='cancelled', updated=?1, completed=?1 \
             WHERE state='running' AND cancel_requested=1",
            params![ts],
        )?;
        let failed = conn.execute(
            "UPDATE operations SET state='failed', updated=?1, completed=?1, \
             error='interrupted by restart; retry limit reached' \
             WHERE state='running' AND attempts >= max_attempts",
            params![ts],
        )?;
        let requeued = conn.execute(
            "UPDATE operations SET state='queued', updated=?1 WHERE state='running'",
            params![ts],
        )?;
        Ok(cancelled + failed + requeued)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> OperationsDb {
        OperationsDb::open(":memory:").unwrap()
    }

    fn new_op(key: Option<&str>) -> NewOperation {
        let mut n = NewOperation::new("golden-image", "vm-a", "default");
        n.idempotency_key = key.map(String::from);
        n.max_attempts = 2;
        n
    }

    #[test]
    fn lifecycle_reaches_succeeded_with_result() {
        let db = db();
        let (op, created) = db.create(new_op(None)).unwrap();
        assert!(created);
        assert_eq!(op.state, OpState::Queued);
        assert!(db.start(&op.id).unwrap());
        db.set_progress(&op.id, "cloning", 250).unwrap();
        let mid = db.get(&op.id).unwrap().unwrap();
        assert_eq!(
            (mid.state, mid.progress, mid.attempts),
            (OpState::Running, 99, 1)
        );
        db.succeed(&op.id, Some(&serde_json::json!({"dv": "img"})))
            .unwrap();
        let done = db.get(&op.id).unwrap().unwrap();
        assert_eq!(done.state, OpState::Succeeded);
        assert_eq!(done.progress, 100);
        assert_eq!(done.result.unwrap()["dv"], "img");
        assert!(done.completed.is_some());
    }

    #[test]
    fn idempotency_key_dedupes() {
        let db = db();
        let (a, created_a) = db.create(new_op(Some("k1"))).unwrap();
        let (b, created_b) = db.create(new_op(Some("k1"))).unwrap();
        assert!(created_a && !created_b);
        assert_eq!(a.id, b.id);
        let (c, _) = db.create(new_op(Some("k2"))).unwrap();
        assert_ne!(a.id, c.id);
        assert_eq!(db.list(None, 10).unwrap().len(), 2);
    }

    #[test]
    fn start_only_claims_queued_operations_once() {
        let db = db();
        let (op, _) = db.create(new_op(None)).unwrap();
        assert!(db.start(&op.id).unwrap());
        assert!(!db.start(&op.id).unwrap());
    }

    #[test]
    fn retry_limit_ends_in_failed() {
        let db = db();
        let (op, _) = db.create(new_op(None)).unwrap();
        db.start(&op.id).unwrap();
        assert_eq!(db.fail(&op.id, "boom", true).unwrap(), OpState::Queued);
        db.start(&op.id).unwrap();
        // attempts (2) == max_attempts (2): no more retries.
        assert_eq!(
            db.fail(&op.id, "boom again", true).unwrap(),
            OpState::Failed
        );
        let f = db.get(&op.id).unwrap().unwrap();
        assert_eq!(f.error.as_deref(), Some("boom again"));
        assert!(f.completed.is_some());
    }

    #[test]
    fn non_retryable_failure_is_final() {
        let db = db();
        let (op, _) = db.create(new_op(None)).unwrap();
        db.start(&op.id).unwrap();
        assert_eq!(
            db.fail(&op.id, "bad input", false).unwrap(),
            OpState::Failed
        );
    }

    #[test]
    fn cancel_queued_is_immediate_and_running_is_flagged() {
        let db = db();
        let (q, _) = db.create(new_op(None)).unwrap();
        assert_eq!(db.request_cancel(&q.id).unwrap(), Some(OpState::Cancelled));
        assert!(!db.start(&q.id).unwrap());

        let (r, _) = db.create(new_op(None)).unwrap();
        db.start(&r.id).unwrap();
        assert_eq!(db.request_cancel(&r.id).unwrap(), Some(OpState::Running));
        assert!(db.get(&r.id).unwrap().unwrap().cancel_requested);
        // A cancelled-while-running op must not be retried.
        assert_eq!(db.fail(&r.id, "stopped", true).unwrap(), OpState::Failed);
        assert_eq!(db.request_cancel("missing").unwrap(), None);
    }

    #[test]
    fn recover_requeues_running_and_fails_exhausted() {
        let db = db();
        let (a, _) = db.create(new_op(None)).unwrap();
        db.start(&a.id).unwrap(); // attempts 1 of 2
        let (b, _) = db.create(new_op(None)).unwrap();
        db.start(&b.id).unwrap();
        db.fail(&b.id, "x", true).unwrap();
        db.start(&b.id).unwrap(); // attempts 2 of 2
        let (c, _) = db.create(new_op(None)).unwrap();
        db.start(&c.id).unwrap();
        db.request_cancel(&c.id).unwrap();

        assert_eq!(db.recover_interrupted().unwrap(), 3);
        assert_eq!(db.get(&a.id).unwrap().unwrap().state, OpState::Queued);
        assert_eq!(db.get(&b.id).unwrap().unwrap().state, OpState::Failed);
        assert_eq!(db.get(&c.id).unwrap().unwrap().state, OpState::Cancelled);
        assert_eq!(db.list_active().unwrap().len(), 1);
    }

    #[test]
    fn orphan_requeue_respects_attempt_limit() {
        let db = db();
        let (op, _) = db.create(new_op(None)).unwrap();
        db.start(&op.id).unwrap(); // attempt 1 of 2
        db.touch(&op.id).unwrap();
        assert_eq!(db.requeue_orphan(&op.id).unwrap(), OpState::Queued);
        db.start(&op.id).unwrap(); // attempt 2 of 2
        assert_eq!(db.requeue_orphan(&op.id).unwrap(), OpState::Failed);
    }

    #[test]
    fn ops_db_path_prefers_explicit_then_sits_next_to_the_auth_db() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            ops_db_path(env(&[
                ("ZORVIA_OPS_DB", "/x/o.db"),
                ("ZORVIA_AUTH_DB", "/data/auth.db")
            ])),
            "/x/o.db"
        );
        assert_eq!(
            ops_db_path(env(&[("ZORVIA_AUTH_DB", "/data/auth.db")])),
            "/data/ops.db"
        );
        // A bare file name has no directory to reuse: fall back to the data dir.
        assert!(ops_db_path(env(&[("ZORVIA_AUTH_DB", "auth.db")])).ends_with("zorvia/ops.db"));
        assert!(ops_db_path(env(&[])).ends_with("zorvia/ops.db"));
    }

    #[test]
    fn state_survives_reopen() {
        let dir = std::env::temp_dir().join(format!("zorvia-ops-{}", uuid::Uuid::new_v4()));
        let path = dir.join("ops.db").to_string_lossy().to_string();
        let id = {
            let db = OperationsDb::open(&path).unwrap();
            let (op, _) = db.create(new_op(Some("persist"))).unwrap();
            db.start(&op.id).unwrap();
            op.id
        };
        let db = OperationsDb::open(&path).unwrap();
        assert_eq!(db.get(&id).unwrap().unwrap().state, OpState::Running);
        db.recover_interrupted().unwrap();
        assert_eq!(db.get(&id).unwrap().unwrap().state, OpState::Queued);
        std::fs::remove_dir_all(dir).ok();
    }
}
