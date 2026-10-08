//! Linux 管理 socket 协议：与按键协议分开版本化，设置程序经它查状态、请求重载、读统计。
//! 只暴露这里的枚举能力，不承载任意命令或文件路径。
//!
//! 类型原本在 `apps/linux/server/src/protocol/admin/`，P3 的设置应用要共用同一套 schema，
//! 上移到这里，Server 侧只做 re-export。

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

/// 管理 socket 路径：`QINGJIAN_ADMIN_SOCKET` 优先；否则与按键 socket 同目录的
/// `qingjian-admin.sock`（`QINGJIAN_SOCKET` 覆盖按键路径时跟着走），缺省
/// `$XDG_RUNTIME_DIR/qingjian-admin.sock`，无 runtime 目录时用按用户隔离的 `/tmp/qingjian-<euid>/`。
#[cfg(unix)]
pub fn socket_path() -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Some(path) = std::env::var_os("QINGJIAN_ADMIN_SOCKET") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("QINGJIAN_SOCKET") {
        return PathBuf::from(path).with_file_name("qingjian-admin.sock");
    }
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/qingjian-{}", unsafe { libc::geteuid() })))
        .join("qingjian-admin.sock")
}
