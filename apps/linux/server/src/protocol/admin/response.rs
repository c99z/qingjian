//! 管理响应：按命令一一对应，外加明确的错误回包。
use serde::{Deserialize, Serialize};

use super::{ReloadReport, StatsReport, StatusReport};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminResponse {
    Status(StatusReport),

    Reload(ReloadReport),

    Stats(StatsReport),

    /// 协议版本不符、请求无法解析或服务不可用；给用户看的中文原因。
    Error(String),
}
