//! 登录响应，本机凭据文件持久化会话。
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct AccountSession {
    pub username: String,

    pub token: String,

    pub revision: u64,

    pub expires_at: u64,

    #[serde(default)]
    pub server_url: String,
}
