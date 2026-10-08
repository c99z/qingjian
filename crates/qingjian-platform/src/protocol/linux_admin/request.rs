//! 管理请求：版本号加一条命令。
use super::AdminCommand;
use serde::{Deserialize, Serialize};

/// 一条管理请求；`version` 必须是 [`super::LINUX_ADMIN_PROTOCOL`]。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRequest {
    pub version: u32,

    pub command: AdminCommand,
}
