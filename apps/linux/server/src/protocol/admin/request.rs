//! 管理请求：版本号加一条命令。
use serde::{Deserialize, Serialize};

/// 一条管理请求；`version` 必须是 [`super::LINUX_ADMIN_PROTOCOL`]。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRequest {
    pub version: u32,

    pub command: AdminCommand,
}

/// 管理接口能力；加新能力时加 variant 并升协议版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminCommand {
    /// 查询版本、配置生效状态、模型与云服务状态。
    Status,

    /// 立即检查已保存配置并应用，返回逐部分结果。
    Reload,

    /// 今日 / 近七天 / 累计输入量与词汇学习统计。
    Stats,
}
