//! HTTP 错误统一映射，不向客户端暴露数据库与凭据详情。
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("invalid request: {0}")]
    Invalid(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("conflict")]
    Conflict,

    #[error("not found")]
    NotFound,

    #[error("rate limited")]
    RateLimited,

    #[error("registration disabled")]
    RegistrationDisabled,

    #[error("server busy")]
    Busy,

    #[error("storage error")]
    Internal,
}
impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        tracing::error!(%error, "数据库操作失败");
        Self::Internal
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, message),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "用户名、密码或会话无效".into()),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "账号已存在，或云端数据已被其他设备更新，请先下载确认".into(),
            ),
            Self::NotFound => (StatusCode::NOT_FOUND, "账号尚无云端备份".into()),
            Self::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "尝试次数过多，请稍后重试".into(),
            ),
            Self::RegistrationDisabled => (StatusCode::FORBIDDEN, "服务已关闭注册".into()),
            Self::Busy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "账号服务繁忙，请稍后重试".into(),
            ),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "账号服务存储失败".into()),
        };
        (status, axum::Json(serde_json::json!({"message": message}))).into_response()
    }
}
