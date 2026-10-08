//! 管理接口状态报告。
mod cloud;
mod config;
mod model;
pub use cloud::CloudStatus;
pub use config::ConfigStatus;
pub use model::ModelStatus;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    pub version: String,

    pub protocol: u32,

    pub config: ConfigStatus,

    pub model: ModelStatus,

    /// Ready 表示已装配，并不代表一次网络请求已成功。
    pub cloud: CloudStatus,
}
