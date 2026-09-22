use anyhow::{Context, Result};
use rusqlite::params;
use sha2::{Digest, Sha256};
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
    pub token_version: u32,
    pub created: String,
    pub last_login: Option<String>,
}

impl User {
    pub fn verify_password(&self, password: &str) -> Result<bool> {
        Ok(bcrypt::verify(password, &self.password_hash)?)
    }
}

#[derive(Debug, Clone)]
pub struct ApiTokenRecord {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub scopes: Vec<String>,
    pub expires_at: Option<String>,
    pub created_by: Option<String>,
    pub created: String,
    pub last_used: Option<String>,
    pub revoked: bool,
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
            );
            CREATE TABLE IF NOT EXISTS api_tokens (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                token_hash TEXT UNIQUE NOT NULL,
                role TEXT NOT NULL,
                scopes TEXT NOT NULL DEFAULT '[]',
                expires_at TEXT,
                created_by TEXT,
                created TEXT NOT NULL,
                last_used TEXT,
                revoked INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        // Added after the table above shipped -- ALTER TABLE ADD COLUMN on an
        // existing DB rather than baking into CREATE TABLE, since sqlite's
        // IF NOT EXISTS only guards table creation, not column additions.
        let _ =
            conn.execute_batch("ALTER TABLE users ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;");
        let _ = conn.execute_batch("ALTER TABLE users ADD COLUMN last_login TEXT;");
        let _ = conn.execute_batch(
            "ALTER TABLE users ADD COLUMN token_version INTEGER NOT NULL DEFAULT 0;",
        );
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
            "INSERT INTO users (id, username, password_hash, role, created, token_version) VALUES (?1,?2,?3,?4,?5,0)",
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
            token_version: 0,
            created,
            last_login: None,
        })
    }

    pub fn get_by_username(&self, username: &str) -> Result<Option<User>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, username, password_hash, role, totp_secret, totp_enabled, enabled, created, last_login, COALESCE(token_version, 0) FROM users WHERE username = ?1",
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
            "SELECT id, username, password_hash, role, totp_secret, totp_enabled, enabled, created, last_login, COALESCE(token_version, 0) FROM users WHERE id = ?1",
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
            "SELECT id, username, password_hash, role, totp_secret, totp_enabled, enabled, created, last_login, COALESCE(token_version, 0) FROM users ORDER BY created ASC",
        )?;
        let rows = stmt.query_map([], row_to_user)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

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
            "UPDATE users SET role = ?1, token_version = token_version + 1 WHERE id = ?2",
            params![role_to_str(&role), id],
        )
        .context("update role")?;
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        if enabled {
            conn.execute("UPDATE users SET enabled = 1 WHERE id = ?1", params![id])
                .context("set enabled")?;
        } else {
            conn.execute(
                "UPDATE users SET enabled = 0, token_version = token_version + 1 WHERE id = ?1",
                params![id],
            )
            .context("set enabled")?;
        }
        Ok(())
    }

    pub fn bump_token_version(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE users SET token_version = token_version + 1 WHERE id = ?1",
            params![id],
        )
        .context("bump token_version")?;
        Ok(())
    }

    pub fn update_password(&self, id: &str, password: &str) -> Result<()> {
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE users SET password_hash = ?1, token_version = token_version + 1 WHERE id = ?2",
            params![hash, id],
        )
        .context("update password")?;
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
            "UPDATE users SET totp_secret = NULL, totp_enabled = 0, token_version = token_version + 1 WHERE id = ?1",
            params![user_id],
        )?;
        Ok(())
    }

    // ── API tokens ───────────────────────────────────────────────

    pub fn hash_api_token(plaintext: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(plaintext.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Creates a token; returns `(record, plaintext)` — plaintext shown once.
    pub fn create_api_token(
        &self,
        name: &str,
        role: Role,
        scopes: Vec<String>,
        expires_at: Option<&str>,
        created_by: Option<&str>,
    ) -> Result<(ApiTokenRecord, String)> {
        let id = uuid::Uuid::new_v4().to_string();
        let plaintext = format!("zrv_{}", uuid::Uuid::new_v4());
        let token_hash = Self::hash_api_token(&plaintext);
        let created = chrono::Utc::now().to_rfc3339();
        let scopes_json = serde_json::to_string(&scopes)?;
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "INSERT INTO api_tokens (id, name, token_hash, role, scopes, expires_at, created_by, created, revoked)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,0)",
            params![
                id,
                name,
                token_hash,
                role_to_str(&role),
                scopes_json,
                expires_at,
                created_by,
                created
            ],
        )?;
        Ok((
            ApiTokenRecord {
                id,
                name: name.to_string(),
                role,
                scopes,
                expires_at: expires_at.map(|s| s.to_string()),
                created_by: created_by.map(|s| s.to_string()),
                created,
                last_used: None,
                revoked: false,
            },
            plaintext,
        ))
    }

    pub fn lookup_api_token(&self, plaintext: &str) -> Result<Option<ApiTokenRecord>> {
        let hash = Self::hash_api_token(plaintext);
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, role, scopes, expires_at, created_by, created, last_used, revoked
             FROM api_tokens WHERE token_hash = ?1",
        )?;
        let mut rows = stmt.query(params![hash])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let rec = row_to_api_token(row)?;
        if rec.revoked {
            return Ok(None);
        }
        if let Some(ref exp) = rec.expires_at {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(exp) {
                if dt < chrono::Utc::now() {
                    return Ok(None);
                }
            }
        }
        Ok(Some(rec))
    }

    pub fn touch_api_token(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        conn.execute(
            "UPDATE api_tokens SET last_used = ?1 WHERE id = ?2",
            params![chrono::Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn list_api_tokens(&self) -> Result<Vec<ApiTokenRecord>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, role, scopes, expires_at, created_by, created, last_used, revoked
             FROM api_tokens ORDER BY created ASC",
        )?;
        let rows = stmt.query_map([], row_to_api_token)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub fn revoke_api_token(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let n = conn.execute(
            "UPDATE api_tokens SET revoked = 1 WHERE id = ?1 AND revoked = 0",
            params![id],
        )?;
        Ok(n > 0)
    }

    pub fn delete_api_token(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        let n = conn.execute("DELETE FROM api_tokens WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }
}

fn role_to_str(role: &Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::User => "user",
        Role::Viewer => "viewer",
    }
}

fn parse_role_str(s: &str) -> Role {
    match s {
        "admin" => Role::Admin,
        "viewer" => Role::Viewer,
        _ => Role::User,
    }
}

fn row_to_user(row: &rusqlite::Row) -> rusqlite::Result<User> {
    let role_s: String = row.get(3)?;
    let totp_enabled: i64 = row.get(5)?;
    let enabled: i64 = row.get(6)?;
    let token_version: i64 = row.get(9)?;
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        password_hash: row.get(2)?,
        role: parse_role_str(&role_s),
        totp_secret: row.get(4)?,
        totp_enabled: totp_enabled != 0,
        enabled: enabled != 0,
        created: row.get(7)?,
        last_login: row.get(8)?,
        token_version: token_version as u32,
    })
}

fn row_to_api_token(row: &rusqlite::Row) -> rusqlite::Result<ApiTokenRecord> {
    let role_s: String = row.get(2)?;
    let scopes_s: String = row.get(3)?;
    let scopes: Vec<String> = serde_json::from_str(&scopes_s).unwrap_or_default();
    let revoked: i64 = row.get(8)?;
    Ok(ApiTokenRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        role: parse_role_str(&role_s),
        scopes,
        expires_at: row.get(4)?,
        created_by: row.get(5)?,
        created: row.get(6)?,
        last_used: row.get(7)?,
        revoked: revoked != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_version_bumps_on_disable() {
        let db = UserDb::open(":memory:").unwrap();
        let u = db.create_user("alice", "password123", Role::User).unwrap();
        assert_eq!(u.token_version, 0);
        db.set_enabled(&u.id, false).unwrap();
        let u2 = db.get_by_id(&u.id).unwrap().unwrap();
        assert!(!u2.enabled);
        assert_eq!(u2.token_version, 1);
    }

    #[test]
    fn api_token_roundtrip() {
        let db = UserDb::open(":memory:").unwrap();
        let (rec, plain) = db
            .create_api_token("ci", Role::User, vec!["vm.read".into()], None, None)
            .unwrap();
        let found = db.lookup_api_token(&plain).unwrap().unwrap();
        assert_eq!(found.id, rec.id);
        assert_eq!(found.role, Role::User);
        db.revoke_api_token(&rec.id).unwrap();
        assert!(db.lookup_api_token(&plain).unwrap().is_none());
    }
}
