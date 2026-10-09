//! 自建账号服务启动入口，默认仅监听回环地址；容器里由环境变量改成 `0.0.0.0`。
//! 子命令 `healthcheck` 给容器健康检查用（镜像里没有 curl），`backup <文件>` 做运行中一致性备份。
use qingjian_account_server::{http, probe, store::AccountStore};
use std::io::IsTerminal;
use std::path::PathBuf;

const USAGE: &str = "用法：qingjian-account-server [healthcheck | backup <目标文件> | --version]
环境变量：QINGJIAN_ACCOUNT_BIND（默认 127.0.0.1:9080）、QINGJIAN_ACCOUNT_DB（默认 account-data/accounts.sqlite3）、
QINGJIAN_REGISTRATION=open 开放注册、RUST_LOG 日志级别（默认 info）";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("qingjian-account-server: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bind = std::env::var("QINGJIAN_ACCOUNT_BIND").unwrap_or_else(|_| "127.0.0.1:9080".into());
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => {}
        ["--version"] => {
            println!("qingjian-account-server {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        ["healthcheck"] => return Ok(probe::health(&bind)?),
        ["backup", dest] => {
            init_tracing();
            set_umask();
            // 不能顺手建个空库：路径写错时那会是一份「成功」的空备份。
            let db = database();
            if !db.is_file() {
                return Err(format!("数据库不存在：{}", db.display()).into());
            }
            AccountStore::open(&db, false)?.backup_to(dest.as_ref())?;
            println!("已备份到 {dest}");
            return Ok(());
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
    init_tracing();
    set_umask();
    let db = database();
    let registration = std::env::var("QINGJIAN_REGISTRATION").is_ok_and(|v| v == "open");
    let store = AccountStore::open(&db, registration)?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, db = %db.display(), registration, "青简账号服务启动");
    axum::serve(listener, http::router(store))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("青简账号服务已停止");
    Ok(())
}
fn database() -> PathBuf {
    std::env::var_os("QINGJIAN_ACCOUNT_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("account-data/accounts.sqlite3"))
}
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    // docker logs / journald 不是终端，不输出颜色控制符。
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(std::io::stdout().is_terminal())
        .init();
}
fn set_umask() {
    // 数据库、WAL 与备份文件都只给服务账户读写。
    #[cfg(unix)]
    unsafe {
        libc::umask(0o077);
    }
}
/// 容器里服务是 PID 1，内核不给它装默认的 SIGTERM 处理，不自己接住的话 `docker stop` 要等超时后 SIGKILL。
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("收到停止信号，处理完进行中的请求后退出");
}
