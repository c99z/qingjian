//! 管理客户端的联测：起一个真 UnixListener 当服务端，走完整的长度前缀 JSON 读写路径。

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;

use qingjian_platform::protocol::linux_admin::{
    AdminRequest, AdminResponse, CloudStatus, ConfigStatus, LINUX_ADMIN_PROTOCOL, ModelStatus,
    ReloadReport, StatusReport,
};
use qingjian_platform::protocol::{read_message, write_message};
use qingjian_platform::settings::ComponentStatus;

use super::client::AdminClient;
use super::error::AdminError;

fn temp_socket(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qingjian-settings-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// 起一条假服务连接：断言收到的是 `command`，然后回 `response`。
fn serve_once(
    path: PathBuf,
    expect: &str,
    response: AdminResponse,
) -> thread::JoinHandle<Result<AdminRequest, String>> {
    let listener = UnixListener::bind(&path).unwrap();
    let expect = expect.to_owned();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
        let request: AdminRequest = read_message(&mut stream)
            .map_err(|e| e.to_string())?
            .ok_or("client closed without a request")?;
        let command = serde_json::to_value(&request.command).unwrap();
        if command != serde_json::Value::String(expect) {
            return Err(format!("unexpected command: {command}"));
        }
        write_message(&mut stream, &response).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&path);
        Ok(request)
    })
}

#[test]
fn status_roundtrips_over_a_real_socket() {
    let path = temp_socket("status.sock");
    let report = StatusReport {
        version: "0.1.5-dev".to_owned(),
        protocol: LINUX_ADMIN_PROTOCOL,
        config: ConfigStatus {
            generation: 3,
            pending: true,
            last_error: None,
        },
        model: ModelStatus::Loaded,
        cloud: CloudStatus::Disabled,
    };
    let server = serve_once(
        path.clone(),
        "status",
        AdminResponse::Status(report.clone()),
    );
    let client = AdminClient::with_path(path);
    assert_eq!(client.status().unwrap(), report);
    let request = server.join().unwrap().unwrap();
    assert_eq!(request.version, LINUX_ADMIN_PROTOCOL);
}

#[test]
fn reload_returns_the_server_component_status() {
    let path = temp_socket("reload.sock");
    let server = serve_once(
        path.clone(),
        "reload",
        AdminResponse::Reload(ReloadReport {
            generation: 4,
            server: ComponentStatus::PendingReload,
        }),
    );
    let client = AdminClient::with_path(path);
    assert_eq!(client.reload().unwrap(), ComponentStatus::PendingReload);
    server.join().unwrap().unwrap();
}

#[test]
fn server_error_reply_becomes_rejected() {
    let path = temp_socket("error.sock");
    let server = serve_once(
        path.clone(),
        "status",
        AdminResponse::Error("管理协议版本不符".to_owned()),
    );
    let client = AdminClient::with_path(path);
    assert!(matches!(
        client.status(),
        Err(AdminError::Rejected(message)) if message == "管理协议版本不符"
    ));
    server.join().unwrap().unwrap();
}

#[test]
fn a_mismatched_reply_is_unexpected() {
    let path = temp_socket("mismatch.sock");
    let server = serve_once(
        path.clone(),
        "status",
        AdminResponse::Reload(ReloadReport {
            generation: 0,
            server: ComponentStatus::Applied,
        }),
    );
    let client = AdminClient::with_path(path);
    assert!(matches!(client.status(), Err(AdminError::Unexpected(_))));
    server.join().unwrap().unwrap();
}

#[test]
fn a_missing_socket_means_the_server_is_not_running() {
    let client = AdminClient::with_path(temp_socket("nobody.sock"));
    assert!(matches!(client.status(), Err(AdminError::NotRunning)));
    assert_eq!(AdminError::NotRunning.user_message(), "服务未运行");
}
