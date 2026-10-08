//! 云服务装配状态，网络结果由连接测试和日志报告。
use serde::{Deserialize, Serialize};
/// 云服务装配是否就绪。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum CloudStatus {
    Disabled,

    Ready,

    Failed(String),
}
