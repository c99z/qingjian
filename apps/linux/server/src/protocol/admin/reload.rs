//! `Reload` 命令的回包：应用结果复用共享设置基础的 `ComponentStatus`。
use qingjian_platform::settings::ComponentStatus;
use serde::{Deserialize, Serialize};

/// 一次重载的结果；Fcitx5 侧的结果由设置程序自己走 D-Bus 拿，不经过这里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReloadReport {
    /// 应用后的配置代次；解析失败保持原代次。
    pub generation: u64,

    /// 青简服务的应用结果（`Applied` / `PendingReload` / `Failed`）。
    pub server: ComponentStatus,
}
