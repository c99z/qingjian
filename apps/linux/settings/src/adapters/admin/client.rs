//! 管理 socket 客户端：连上 Server 的管理 socket，发一条命令读一条回包。
//! 同步阻塞 IO（带超时），调用方负责放到后台线程；socket 路径规则在平台层
//! [`qingjian_platform::protocol::linux_admin::socket_path`]，与 Server 共用同一份定义。

use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use qingjian_platform::protocol::linux_admin::{
    AdminCommand, AdminRequest, AdminResponse, LINUX_ADMIN_PROTOCOL, StatusReport,
};
use qingjian_platform::protocol::{read_message, write_message};
use qingjian_platform::settings::ComponentStatus;

use super::error::AdminError;

/// 单次请求的超时；管理操作在 Server 主线程串行，正常是毫秒级，3 秒还没回就当服务卡住。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

pub struct AdminClient {
    path: PathBuf,
}

impl Default for AdminClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminClient {
    /// 按默认规则定位管理 socket。
    pub fn new() -> Self {
        Self::with_path(qingjian_platform::protocol::linux_admin::socket_path())
    }

    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// `Status`：版本、配置代次与挂起状态、模型与云服务状态。
    pub fn status(&self) -> Result<StatusReport, AdminError> {
        match self.request(AdminCommand::Status)? {
            AdminResponse::Status(report) => Ok(report),
            AdminResponse::Error(message) => Err(AdminError::Rejected(message)),
            other => Err(AdminError::Unexpected(format!("{other:?}"))),
        }
    }

    /// `Reload`：让服务立即读盘应用，返回青简服务的应用结果。
    pub fn reload(&self) -> Result<ComponentStatus, AdminError> {
        match self.request(AdminCommand::Reload)? {
            AdminResponse::Reload(report) => Ok(report.server),
            AdminResponse::Error(message) => Err(AdminError::Rejected(message)),
            other => Err(AdminError::Unexpected(format!("{other:?}"))),
        }
    }

    fn request(&self, command: AdminCommand) -> Result<AdminResponse, AdminError> {
        let mut stream = UnixStream::connect(&self.path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused => AdminError::NotRunning,
            _ => AdminError::Io(error),
        })?;
        stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
        stream.set_write_timeout(Some(REQUEST_TIMEOUT))?;
        let request = AdminRequest {
            version: LINUX_ADMIN_PROTOCOL,
            command,
        };
        write_message(&mut stream, &request)?;
        read_message(&mut stream)?.ok_or(AdminError::Closed)
    }
}
