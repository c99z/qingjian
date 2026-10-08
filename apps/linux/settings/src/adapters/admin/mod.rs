//! Server 管理 socket 的客户端：路径解析、连接与请求/回包配对。

mod client;
mod error;

pub use client::AdminClient;
pub use error::AdminError;

#[cfg(test)]
mod tests;
