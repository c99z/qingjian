//! 管理 socket 协议类型：定义已上移到 `qingjian_platform::protocol::linux_admin`（设置应用共用），
//! 这里只做 re-export，保持 server 内部的 `protocol::admin::…` 路径不变。

pub use qingjian_platform::protocol::linux_admin::{
    AdminCommand, AdminRequest, AdminResponse, CloudStatus, ConfigStatus, LINUX_ADMIN_PROTOCOL,
    ModelStatus, ReloadReport, StatsReport, StatusReport, UsageCounts, UsageReport,
    VocabularyReport,
};
