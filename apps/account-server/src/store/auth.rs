//! Argon2id 口令验证和账号锁定；数据库只保存口令哈希和令牌哈希。
use crate::{
    error::ApiError,
    store::{AccountStore, now},
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use qingjian_platform::account::{AccountSession, LoginRequest};
use rusqlite::{OptionalExtension, params};
fn username(value: &str) -> Result<String, ApiError> {
    let value = value.trim().to_ascii_lowercase();
    if !(3..=64).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'@'))
    {
        return Err(ApiError::Invalid(
            "用户名为 3–64 个字母、数字或 _ - . @".into(),
        ));
    }
    Ok(value)
}
impl AccountStore {
    pub fn register(&self, request: LoginRequest) -> Result<AccountSession, ApiError> {
        if !self.registration {
            return Err(ApiError::RegistrationDisabled);
        }
        let username = username(&request.username)?;
        if !(12..=256).contains(&request.password.len()) {
            return Err(ApiError::Invalid("密码须为 12–256 字节".into()));
        }
        let password = Argon2::default()
            .hash_password(request.password.as_bytes())
            .map_err(|_| ApiError::Internal)?
            .to_string();
        let db = self.db.lock().map_err(|_| ApiError::Internal)?;
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE username=?1)",
            [&username],
            |r| r.get(0),
        )?;
        if exists {
            return Err(ApiError::Conflict);
        }
        db.execute(
            "INSERT INTO users(username,password) VALUES (?1,?2)",
            params![username, password],
        )?;
        Self::issue(&db, db.last_insert_rowid(), username)
    }
    pub fn login(&self, request: LoginRequest) -> Result<AccountSession, ApiError> {
        let username = username(&request.username)?;
        if request.password.len() > 256 {
            return Err(ApiError::Unauthorized);
        }
        let db = self.db.lock().map_err(|_| ApiError::Internal)?;
        let found: Option<(i64, String, u32, i64)> = db
            .query_row(
                "SELECT id,password,failures,locked_until FROM users WHERE username=?1",
                [&username],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((user, encoded, failures, locked)) = found else {
            return Err(ApiError::Unauthorized);
        };
        if locked > now() {
            return Err(ApiError::RateLimited);
        }
        let hash = PasswordHash::new(&encoded).map_err(|_| ApiError::Internal)?;
        if Argon2::default()
            .verify_password(request.password.as_bytes(), &hash)
            .is_err()
        {
            let failures = if locked != 0 {
                1
            } else {
                failures.saturating_add(1)
            };
            db.execute(
                "UPDATE users SET failures=?1,locked_until=?2 WHERE id=?3",
                params![failures, if failures >= 5 { now() + 60 } else { 0 }, user],
            )?;
            return Err(ApiError::Unauthorized);
        }
        db.execute(
            "UPDATE users SET failures=0,locked_until=0 WHERE id=?1",
            [user],
        )?;
        Self::issue(&db, user, username)
    }
}
