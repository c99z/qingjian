//! 自建账号服务客户端：严格限定凭据目标、禁用重定向、本机 0600 会话文件。
mod client;
pub use client::AccountClient;
use qingjian_platform::account::AccountSession;
use std::path::Path;

pub fn session_path() -> std::path::PathBuf {
    qingjian_platform::dirs::linux::config_path().with_file_name("account-session.json")
}
pub fn load_session(path: &Path) -> Result<Option<AccountSession>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if !meta.is_file() || meta.len() > 16384 => {
            return Err("本机会话文件不是有效的普通文件".into());
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("本机会话文件不可读".into()),
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("本机会话文件不可读".into()),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "本机会话文件格式损坏".into())
}
pub fn save_session(path: &Path, session: &AccountSession) -> Result<(), String> {
    qingjian_platform::credentials::write_private(
        path,
        &serde_json::to_vec(session).map_err(|_| "会话编码失败".to_owned())?,
    )
    .map_err(|_| "本机会话保存失败".into())
}
