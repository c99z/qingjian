//! 配置生效状态。
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigStatus {
    pub generation: u64,

    pub pending: bool,

    pub last_error: Option<String>,
}
