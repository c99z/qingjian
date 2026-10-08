//! 生效报告：一次「应用」之后青简服务与 Fcitx5 各自的结果。
//! 本阶段只有类型与序列化（走管理 socket 给设置界面），实际填充在服务的重载接入之后。

use serde::{Deserialize, Serialize};

/// 单个组件的应用结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum ComponentStatus {
    /// 已保存：配置写盘成功，等服务重载或下次启动生效（服务没跑也是这个状态）。
    Saved,

    /// 等待重载：现有组句结束后应用。
    PendingReload,

    /// 加载中：模型 / 词库 / 释义表等资源在后台加载，完成后才生效。
    Loading,

    /// 已生效。
    Applied,

    /// 失败，附给用户看的中文原因。
    Failed(String),
}

/// 一次保存的生效报告：青简服务与 Fcitx5 分开报，不把部分成功显示成全部成功。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyReport {
    /// 青简服务的应用结果。
    pub server: ComponentStatus,

    /// Fcitx5（外观）的应用结果。
    pub fcitx5: ComponentStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_roundtrips_through_json() {
        let report = ApplyReport {
            server: ComponentStatus::PendingReload,
            fcitx5: ComponentStatus::Failed("Fcitx5 没在运行".to_owned()),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(serde_json::from_str::<ApplyReport>(&json).unwrap(), report);
    }
}
