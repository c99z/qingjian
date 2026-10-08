//! 自建账号服务的版本化 HTTP 数据类型；与 AI 接口独立。
mod credentials;
mod session;
mod snapshot;
mod upload;
pub use credentials::LoginRequest;
pub use session::AccountSession;
pub use snapshot::RemoteSnapshot;
pub use upload::UploadRequest;
