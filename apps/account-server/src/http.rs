//! v1 注册、登录、注销与用户快照 HTTP 接口。耗时哈希和数据库工作走阻塞线程。
use crate::{error::ApiError, store::AccountStore};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use qingjian_platform::account::{AccountSession, LoginRequest, RemoteSnapshot, UploadRequest};

/// 同时处理的请求上限；口令哈希与快照编解码都吃 CPU 和内存，超出的直接回 503。
const MAX_CONCURRENT: usize = 8;

pub fn router(store: AccountStore) -> Router {
    Router::new()
        .route("/v1/register", post(register))
        .route("/v1/login", post(login))
        .route("/v1/logout", post(logout))
        .route("/v1/snapshot", get(download).put(upload))
        .layer(DefaultBodyLimit::max(
            qingjian_platform::backup::MAX_ARCHIVE_BYTES + 1024,
        ))
        .layer(middleware::from_fn(limit_concurrency))
        // 健康检查在并发限制之外：服务忙时容器编排不该把它判成故障重启。
        .route("/health", get(|| async { "ok" }))
        .with_state(store)
}
async fn limit_concurrency(
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, ApiError> {
    static LIMIT: std::sync::LazyLock<tokio::sync::Semaphore> =
        std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(MAX_CONCURRENT));
    let _permit = LIMIT.try_acquire().map_err(|_| ApiError::Busy)?;
    Ok(next.run(request).await)
}
fn token(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned)
        .ok_or(ApiError::Unauthorized)
}
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| ApiError::Internal)?
}
async fn register(
    State(store): State<AccountStore>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AccountSession>, ApiError> {
    blocking(move || store.register(request)).await.map(Json)
}
async fn login(
    State(store): State<AccountStore>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AccountSession>, ApiError> {
    blocking(move || store.login(request)).await.map(Json)
}
async fn logout(
    State(store): State<AccountStore>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let token = token(&headers)?;
    blocking(move || store.logout(&token)).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn download(
    State(store): State<AccountStore>,
    headers: HeaderMap,
) -> Result<Json<RemoteSnapshot>, ApiError> {
    let token = token(&headers)?;
    blocking(move || store.download(&token)).await.map(Json)
}
async fn upload(
    State(store): State<AccountStore>,
    headers: HeaderMap,
    Json(request): Json<UploadRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = token(&headers)?;
    let revision = blocking(move || store.upload(&token, request)).await?;
    Ok(Json(serde_json::json!({"revision": revision})))
}
