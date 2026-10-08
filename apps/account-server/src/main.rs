//! 自建账号服务启动入口，默认仅监听回环地址。
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--version") {
        println!("qingjian-account-server {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    tracing_subscriber::fmt().with_env_filter("info").init();
    #[cfg(unix)]
    unsafe {
        libc::umask(0o077);
    }
    let bind = std::env::var("QINGJIAN_ACCOUNT_BIND").unwrap_or_else(|_| "127.0.0.1:9080".into());
    let db = std::env::var_os("QINGJIAN_ACCOUNT_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("account-data/accounts.sqlite3"));
    let registration = std::env::var("QINGJIAN_REGISTRATION").is_ok_and(|v| v == "open");
    let store = qingjian_account_server::store::AccountStore::open(&db, registration)?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, registration, "青简账号服务启动");
    axum::serve(listener, qingjian_account_server::http::router(store)).await?;
    Ok(())
}
