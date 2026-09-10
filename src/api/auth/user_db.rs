use anyhow::{Context, Result};
use rusqlite::params;
use std::sync::Mutex;

use super::jwt::Role;

pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: Role,
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub enabled: bool,
    pub created: String,
    pub last_login: Option<String>,
}

impl User {
    pub fn verify_password(&self, password: &str) -> Result<bool> {
        Ok(bcrypt::verify(password, &self.password_hash)?)
    }
}

pub struct UserDb {
    conn: Mutex<rusqlite::Connection>,
}

impl UserDb {
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
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL,
                totp_secret TEXT,
                totp_enabled INTEGER NOT NULL DEFAULT 0,
                created TEXT NOT NULL
            );",
        )?;
        // Added after the table above shipped -- ALTER TABLE ADD COLUMN on an
        // existing DB rather than baking into CREATE TABLE, since sqlite's
        // IF NOT EXISTS only guards table creation, not column additions.
        // Errors here (column already exists) are expected on every restart
        // after the first and are ignored to keep this idempotent.
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN last_login TEXT;");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn from_env() -> Result<Self> {
        let path = std::env::var("ZORVIA_AUTH_DB").unwrap_or_else(|_| {
            dirs::data_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("zorvia")
                .join("auth.db")
                .to_string_lossy()
                .to_string()
        });
        Self::open(&path)
    }

    pub fn count_users(&self) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    pub fn seed_admin(&self, username: &str, password: &str) -> Result<Option<User>> {
        if self.count_users()? > 0 {
            return Ok(None);
        }
        let user = self.create_user(username, password, Role::Admin)?;
        log::info!("Seeded bootstrap admin user '{}'", username);
        Ok(Some(user))
    }

    pub fn create_user(&self, username: &str, password: &str, role: Role) -> Result<User> {
        let id = uuid::Uuid::new_v4().to_string();
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        let role_str = role_to_str(&role);
        let created = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "INSERT INTO users (id, username, password_hash, role, created) VALUES (?1,?2,?3,?4,?5)",
            params![id, username, hash, role_str, created],
        )?;
        Ok(User {
            id,
            username: username.to_string(),
            password_hash: hash,
            role,
            totp_secret: None,
            totp_enabled: false,
            enabled: true,
            created,
            last_login: None,
        })
    }

    pub fn get_by_username(&self, username: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, role, totp_secret, totp_enabled, enabled, created, last_login FROM users WHERE username = ?1",
        )?;
        let mut rows = stmt.query(params![username])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_user(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, role, totp_secret, totp_enabled, enabled, created, last_login FROM users WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_user(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_users(&self) -> Result<Vec<User>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, role, totp_secret, totp_enabled, enabled, created, last_login FROM users ORDER BY created ASC",
        )?;
        let rows = stmt.query_map([], row_to_user)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Number of enabled admins -- used to refuse an action that would leave
    /// the instance with no way to sign in as an administrator.
    pub fn enabled_admin_count(&self) -> Result<usize> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE role = 'admin' AND enabled = 1",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    pub fn delete_user(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute("DELETE FROM users WHERE id = ?1", params![id])
            .context("delete user")?;
        Ok(())
    }

    pub fn update_role(&self, id: &str, role: Role) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE users SET role = ?1 WHERE id = ?2",
            params![role_to_str(&role), id],
        )
        .context("update role")?;
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE users SET enabled = ?1 WHERE id = ?2",
            params![if enabled { 1 } else { 0 }, id],
        )
        .context("set enabled")?;
        Ok(())
    }

    pub fn update_last_login(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE users SET last_login = ?1 WHERE id = ?2",
            params![chrono::Utc::now().to_rfc3339(), id],
        )
        .context("update last_login")?;
        Ok(())
    }

    pub fn set_totp(&self, user_id: &str, secret: &str, enabled: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE users SET totp_secret = ?1, totp_enabled = ?2 WHERE id = ?3",
            params![secret, if enabled { 1 } else { 0 }, user_id],
        )
        .context("update totp")?;
        Ok(())
    }

    pub fn disable_totp(&self, user_id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE users SET totp_secret = NULL, totp_enabled = 0 WHERE id = ?1",
            params![user_id],
        )?;
        Ok(())
    }
}

fn role_to_str(role: &Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::User => "user",
        Role::Viewer => "viewer",
    }
}

fn row_to_user(row: &rusqlite::Row) -> rusqlite::Result<User> {
    let role_s: String = row.get(3)?;
    let role = match role_s.as_str() {
        "admin" => Role::Admin,
        "viewer" => Role::Viewer,
        _ => Role::User,
    };
    let totp_enabled: i64 = row.get(5)?;
    let enabled: i64 = row.get(6)?;
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        password_hash: row.get(2)?,
        role,
        totp_secret: row.get(4)?,
        totp_enabled: totp_enabled != 0,
        enabled: enabled != 0,
        created: row.get(7)?,
        last_login: row.get(8)?,
    })
}
