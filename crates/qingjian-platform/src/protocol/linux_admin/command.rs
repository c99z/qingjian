//! Linux 管理 socket 允许的能力。

use serde::{Deserialize, Serialize};

/// 管理接口能力；加新能力时加 variant 并升协议版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminCommand {
    /// 查询版本、配置生效状态、模型与云服务状态。
    Status,

    /// 立即检查已保存配置并应用，返回逐部分结果。
    Reload,

    /// 今日 / 近七天 / 累计输入量与词汇学习统计。
    Stats,

    /// 串行清空输入日志并重新打开写入端，不影响当前组句与学习统计。
    ClearInputLog,

    Backup,

    Restore {
        transfer: String,
    },
}
