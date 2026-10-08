//! 状态报告：版本、配置生效状态、模型与云服务状态。
use serde::{Deserialize, Serialize};

/// `Status` 命令的回包。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    /// 服务版本号。
    pub version: String,

    /// 管理协议版本。
    pub protocol: u32,

    /// 配置生效状态。
    pub config: ConfigStatus,

    /// 本地整句模型状态。
    pub model: ModelStatus,

    /// 云服务状态；本阶段云未接入，恒为 `Disabled` 占位。
    pub cloud: CloudStatus,
}

/// 配置生效状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigStatus {
    /// 当前配置代次，每次成功应用递增。
    pub generation: u64,

    /// 有没有等组句结束才应用的修改。
    pub pending: bool,

    /// 上次重载 / 解析的错误；无错误为 `None`。
    pub last_error: Option<String>,
}

/// 本地整句模型状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum ModelStatus {
    /// 未启用（`[model] enabled = false`）。
    Disabled,

    /// 后台加载中。
    Loading,

    /// 已加载。
    Loaded,

    /// 加载失败或没有模型文件，附原因。
    Failed(String),
}

/// 云服务状态；本阶段只有关闭一种，接入后加 variant。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CloudStatus {
    Disabled,
}
