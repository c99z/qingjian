//! 注册或登录正文，不实现 Debug，避免密码进入诊断。
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub username: String,

    pub password: String,
}
