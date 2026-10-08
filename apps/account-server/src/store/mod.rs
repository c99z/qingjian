//! SQLite 账号、带有效期的哈希令牌和按账号隔离的用户快照。
use crate::error::ApiError;
use qingjian_platform::account::{AccountSession, RemoteSnapshot, UploadRequest};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::{Arc, Mutex};
mod auth;

#[derive(Clone)]
pub struct AccountStore {
    db: Arc<Mutex<Connection>>,

    pub registration: bool,
}
impl AccountStore {
    pub fn open(path: &Path, registration: bool) -> Result<Self, ApiError> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            #[cfg(unix)]
            let newly_created = !parent.exists();
            std::fs::create_dir_all(parent).map_err(|_| ApiError::Internal)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if newly_created {
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                        .map_err(|_| ApiError::Internal)?;
                }
            }
        }
        let db = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| ApiError::Internal)?;
        }
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
          CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, username TEXT NOT NULL UNIQUE, password TEXT NOT NULL, failures INTEGER NOT NULL DEFAULT 0, locked_until INTEGER NOT NULL DEFAULT 0);
          CREATE TABLE IF NOT EXISTS sessions (token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id), expires INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS snapshots (user_id INTEGER PRIMARY KEY REFERENCES users(id), revision INTEGER NOT NULL, archive BLOB NOT NULL);")?;
        Ok(Self {
            db: Arc::new(Mutex::new(db)),
            registration,
        })
    }
    fn user_id(db: &Connection, token: &str) -> Result<i64, ApiError> {
        if token.len() != 64 {
            return Err(ApiError::Unauthorized);
        }
        db.query_row(
            "SELECT user_id FROM sessions WHERE token_hash=?1 AND expires>?2",
            params![token_hash(token), now()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(ApiError::Unauthorized)
    }
    pub fn logout(&self, token: &str) -> Result<(), ApiError> {
        let db = self.db.lock().map_err(|_| ApiError::Internal)?;
        Self::user_id(&db, token)?;
        db.execute(
            "DELETE FROM sessions WHERE token_hash=?1",
            [token_hash(token)],
        )?;
        Ok(())
    }
    pub fn download(&self, token: &str) -> Result<RemoteSnapshot, ApiError> {
        let db = self.db.lock().map_err(|_| ApiError::Internal)?;
        let user = Self::user_id(&db, token)?;
        let (revision, bytes): (i64, Vec<u8>) = db
            .query_row(
                "SELECT revision,archive FROM snapshots WHERE user_id=?1",
                [user],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(ApiError::NotFound)?;
        let archive = serde_json::from_slice(&bytes).map_err(|_| ApiError::Internal)?;
        Ok(RemoteSnapshot {
            revision: u64::try_from(revision).map_err(|_| ApiError::Internal)?,
            archive,
        })
    }
    pub fn upload(&self, token: &str, request: UploadRequest) -> Result<u64, ApiError> {
        request.archive.validate().map_err(ApiError::Invalid)?;
        let bytes = serde_json::to_vec(&request.archive).map_err(|_| ApiError::Internal)?;
        if bytes.len() > qingjian_platform::backup::MAX_ARCHIVE_BYTES {
            return Err(ApiError::Invalid("快照超过 96 MiB".into()));
        }
        let mut db = self.db.lock().map_err(|_| ApiError::Internal)?;
        let transaction = db.transaction()?;
        let user = Self::user_id(&transaction, token)?;
        let revision: i64 = transaction
            .query_row(
                "SELECT revision FROM snapshots WHERE user_id=?1",
                [user],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if u64::try_from(revision).map_err(|_| ApiError::Internal)? != request.expected_revision {
            return Err(ApiError::Conflict);
        }
        let next = revision.checked_add(1).ok_or(ApiError::Internal)?;
        transaction.execute("INSERT INTO snapshots(user_id,revision,archive) VALUES (?1,?2,?3) ON CONFLICT(user_id) DO UPDATE SET revision=excluded.revision,archive=excluded.archive", params![user, next, bytes])?;
        transaction.commit()?;
        Ok(next as u64)
    }
    fn issue(db: &Connection, user: i64, username: String) -> Result<AccountSession, ApiError> {
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let expires_at = now() + 30 * 86400;
        db.execute("DELETE FROM sessions WHERE expires<=?1", [now()])?;
        db.execute(
            "INSERT INTO sessions VALUES (?1,?2,?3)",
            params![token_hash(&token), user, expires_at],
        )?;
        let revision: i64 = db
            .query_row(
                "SELECT revision FROM snapshots WHERE user_id=?1",
                [user],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        Ok(AccountSession {
            token,
            username,
            revision: revision as u64,
            expires_at: expires_at as u64,
            server_url: String::new(),
        })
    }
}
fn token_hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
