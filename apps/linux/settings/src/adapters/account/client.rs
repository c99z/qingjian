//! 阻塞 HTTP 账号客户端，GUI 在后台线程调用。
use qingjian_platform::account::{AccountSession, LoginRequest, RemoteSnapshot, UploadRequest};
use qingjian_platform::backup::Archive;
use reqwest::blocking::{Client, Response};
use reqwest::{StatusCode, Url};
use std::io::Read;

pub struct AccountClient {
    base: String,

    http: Client,
}
impl AccountClient {
    pub fn new(base: &str) -> Result<Self, String> {
        let url = Url::parse(base.trim()).map_err(|_| "账号服务地址不合法".to_owned())?;
        let local = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !(url.scheme() == "https" || (url.scheme() == "http" && local))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "账号服务须使用 HTTPS；仅本机地址允许 HTTP，地址不能包含凭据或查询参数".into(),
            );
        }
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|_| "无法创建账号连接".to_owned())?;
        Ok(Self {
            base: base.trim().trim_end_matches('/').to_owned(),
            http,
        })
    }
    pub fn login(
        &self,
        username: String,
        password: String,
        register: bool,
    ) -> Result<AccountSession, String> {
        let response = self
            .http
            .post(format!(
                "{}/v1/{}",
                self.base,
                if register { "register" } else { "login" }
            ))
            .json(&LoginRequest { username, password })
            .send()
            .map_err(|_| "无法连接账号服务，请检查地址和网络".to_owned())?;
        let mut session: AccountSession = decode(response)?;
        session.server_url = self.base.clone();
        Ok(session)
    }
    pub fn logout(&self, session: &AccountSession) -> Result<(), String> {
        let response = self
            .http
            .post(format!("{}/v1/logout", self.base))
            .bearer_auth(&session.token)
            .send()
            .map_err(|_| "账号服务连接失败，未注销".to_owned())?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Ok(());
        }
        checked(response).map(|_| ())
    }
    pub fn upload(&self, session: &AccountSession, archive: Archive) -> Result<u64, String> {
        archive.validate()?;
        let response = self
            .http
            .put(format!("{}/v1/snapshot", self.base))
            .bearer_auth(&session.token)
            .json(&UploadRequest {
                expected_revision: session.revision,
                archive,
            })
            .send()
            .map_err(|_| "上传失败，云端结果未知；请重新登录核对版本".to_owned())?;
        let value: serde_json::Value = decode(response)?;
        value
            .get("revision")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "账号服务回复缺少版本号".into())
    }
    pub fn download(&self, session: &AccountSession) -> Result<RemoteSnapshot, String> {
        let response = self
            .http
            .get(format!("{}/v1/snapshot", self.base))
            .bearer_auth(&session.token)
            .send()
            .map_err(|_| "下载失败，请检查账号服务和网络".to_owned())?;
        let snapshot: RemoteSnapshot = decode(response)?;
        snapshot.archive.validate()?;
        Ok(snapshot)
    }
}
fn checked(response: Response) -> Result<Response, String> {
    if response.status().is_success() {
        return Ok(response);
    }
    Err(match response.status() {
        StatusCode::UNAUTHORIZED => "用户名、密码或会话无效，请重新登录",
        StatusCode::CONFLICT => "账号已存在，或云端数据已更新；请先下载确认再上传",
        StatusCode::NOT_FOUND => "账号尚无云端备份",
        StatusCode::FORBIDDEN => "账号服务未开放注册",
        StatusCode::TOO_MANY_REQUESTS => "尝试次数过多，请稍后重试",
        StatusCode::BAD_REQUEST => {
            "内容不合法：用户名需为 3–64 个字母、数字或 _ - . @，密码至少 12 字节；请检查备份格式"
        }
        _ => "账号服务返回错误，操作未确认",
    }
    .into())
}
fn decode<T: serde::de::DeserializeOwned>(response: Response) -> Result<T, String> {
    let response = checked(response)?;
    let mut bytes = Vec::new();
    response
        .take(qingjian_platform::backup::MAX_ARCHIVE_BYTES as u64 + 1025)
        .read_to_end(&mut bytes)
        .map_err(|_| "账号服务回复读取失败".to_owned())?;
    if bytes.len() > qingjian_platform::backup::MAX_ARCHIVE_BYTES + 1024 {
        return Err("账号服务回复超过大小上限".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "账号服务回复格式不合法".into())
}
