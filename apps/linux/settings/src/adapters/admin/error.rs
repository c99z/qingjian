//! 管理 socket 客户端的出错类型；`NotRunning` 单列：服务没跑不是失败，保存照常、提示「下次启动生效」。

use qingjian_platform::protocol::CodecError;

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    /// socket 不存在或拒绝连接：Server 没在运行。
    #[error("server is not running")]
    NotRunning,

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("codec: {0}")]
    Codec(#[from] CodecError),

    /// 对端没回包就断开了。
    #[error("connection closed without a reply")]
    Closed,

    /// 服务回了 `Error`（协议版本不符等），附服务给的中文原因。
    #[error("server rejected the request: {0}")]
    Rejected(String),

    /// 回包与请求的命令对不上。
    #[error("unexpected reply: {0}")]
    Unexpected(String),
}

impl AdminError {
    /// 给用户看的中文说明（`NotRunning` 不该走这里——它对应「已保存，下次启动生效」）。
    pub fn user_message(&self) -> String {
        match self {
            Self::NotRunning => "服务未运行".to_owned(),
            Self::Io(error) => format!("连接服务失败：{error}"),
            Self::Codec(error) => format!("与服务通信失败：{error}"),
            Self::Closed => "服务没有回应".to_owned(),
            Self::Rejected(message) => format!("服务拒绝了请求：{message}"),
            Self::Unexpected(_) => "服务的回应无法识别（版本可能不配套）".to_owned(),
        }
    }
}
