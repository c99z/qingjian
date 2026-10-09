//! Argon2id 口令验证和账号锁定；数据库只保存口令哈希和令牌哈希。
//! 口令哈希与校验在数据库锁外做，否则一次登录要把所有请求挡住几十毫秒。
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
use std::sync::LazyLock;

/// 连续错误这么多次后锁定账号。
const MAX_FAILURES: u32 = 5;

/// 锁定时长（秒）。
const LOCK_SECONDS: i64 = 60;

/// 用户名不存在时也校验一次，响应时间不暴露账号是否存在。
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    Argon2::default()
        .hash_password(uuid::Uuid::new_v4().as_bytes())
        .map(|hash| hash.to_string())
        .unwrap_or_default()
});

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
fn verify(password: &str, encoded: &str) -> Result<bool, ApiError> {
    let hash = PasswordHash::new(encoded).map_err(|_| ApiError::Internal)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &hash)
        .is_ok())
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
        let inserted = db.execute(
            "INSERT INTO users(username,password) VALUES (?1,?2) ON CONFLICT(username) DO NOTHING",
            params![username, password],
        )?;
        if inserted == 0 {
            return Err(ApiError::Conflict);
        }
        Self::issue(&db, db.last_insert_rowid(), username)
    }
    pub fn login(&self, request: LoginRequest) -> Result<AccountSession, ApiError> {
        let username = username(&request.username)?;
        if request.password.len() > 256 {
            return Err(ApiError::Unauthorized);
        }
        let found: Option<(i64, String, i64)> = {
            let db = self.db.lock().map_err(|_| ApiError::Internal)?;
            db.query_row(
                "SELECT id,password,locked_until FROM users WHERE username=?1",
                [&username],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
        };
        let Some((user, encoded, locked)) = found else {
            let _ = verify(&request.password, &DUMMY_HASH);
            return Err(ApiError::Unauthorized);
        };
        if locked > now() {
            return Err(ApiError::RateLimited);
        }
        let valid = verify(&request.password, &encoded)?;
        let db = self.db.lock().map_err(|_| ApiError::Internal)?;
        if !valid {
            // 锁定过期后重新计数；计数在锁内读改写，并发的错误登录不会少记。
            db.execute(
                "UPDATE users SET
                   failures = CASE WHEN locked_until != 0 THEN 1 ELSE failures + 1 END,
                   locked_until = CASE WHEN (CASE WHEN locked_until != 0 THEN 1 ELSE failures + 1 END) >= ?1
                                       THEN ?2 ELSE 0 END
                 WHERE id = ?3",
                params![MAX_FAILURES, now() + LOCK_SECONDS, user],
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
