//! 管理 socket 协议：与按键协议分开版本化，设置程序经它查状态、请求重载、读统计。
//! 只暴露这里的枚举能力，不承载任意命令或文件路径。

mod reload;
mod request;
mod response;
mod stats;
mod status;

pub use reload::ReloadReport;
pub use request::{AdminCommand, AdminRequest};
pub use response::AdminResponse;
pub use stats::{StatsReport, UsageCounts, UsageReport, VocabularyReport};
pub use status::{CloudStatus, ConfigStatus, ModelStatus, StatusReport};

/// 管理协议版本；请求带版本，不符明确拒绝。
pub const LINUX_ADMIN_PROTOCOL: u32 = 1;
