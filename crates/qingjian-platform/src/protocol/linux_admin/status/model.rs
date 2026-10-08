//! 本地模型加载状态。
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "message", rename_all = "snake_case")]
pub enum ModelStatus {
    Disabled,

    Loading,

    Loaded,

    Failed(String),
}
