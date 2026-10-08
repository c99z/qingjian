//! 管理 socket：权限、协议版本拒绝、Status / Reload / Stats 往返。
#![cfg(target_os = "linux")]
#[path = "../support/server.rs"]
mod server;
use qingjian_linux_server::protocol::admin::LINUX_ADMIN_PROTOCOL;
use qingjian_platform::protocol::{PROTOCOL_VERSION, read_message, write_message};
use serde_json::{Value, json};
use server::Server;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;

fn admin(stream: &mut UnixStream, command: &str) -> Value {
    write_message(
        stream,
        &json!({"version": LINUX_ADMIN_PROTOCOL, "command": command}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap().unwrap()
}

fn open(stream: &mut UnixStream, id: u64, context: &str) {
    write_message(
        stream,
        &json!({"OpenSession": {"session": id, "app": "admin-test", "protocol": PROTOCOL_VERSION}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap();
    write_message(
        stream,
        &json!({"LinuxHello": {"version": 3, "session": id, "generation": 1, "context": context}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap();
    write_message(
        stream,
        &json!({"LinuxEvent": {"session": id, "event": {"Capabilities": {"sensitive": false, "password": false, "disabled": false}}}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap();
}

fn key(stream: &mut UnixStream, id: u64, character: char) -> Value {
    write_message(
        stream,
        &json!({"LinuxEvent": {"session": id, "event": {"Key": {"event": {"virtual_key": character as u32, "character": character, "modifiers": {"ctrl": false, "shift": false, "alt": false, "win": false, "caps": false, "english_mode": false}}, "release": false}}}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap().unwrap()
}

#[test]
fn admin_socket_is_private_and_rejects_version_mismatch() {
    let directory = std::env::temp_dir().join(format!("qingjian-admin-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut stream = server.connect_admin();
    assert_eq!(
        std::fs::metadata(directory.join("admin.sock"))
            .unwrap()
            .mode()
            & 0o777,
        0o600
    );
    // 版本不符：明确拒绝并断开
    write_message(&mut stream, &json!({"version": 99, "command": "status"})).unwrap();
    let response = read_message::<_, Value>(&mut stream).unwrap().unwrap();
    assert!(
        response["Error"].as_str().unwrap().contains("版本"),
        "{response}"
    );
    assert!(read_message::<_, Value>(&mut stream).unwrap().is_none());
    // 无法解析的命令同样拒绝；其他连接不受影响
    let mut bad = server.connect_admin();
    write_message(&mut bad, &json!({"version": 1, "command": "bogus"})).unwrap();
    assert!(read_message::<_, Value>(&mut bad).unwrap().unwrap()["Error"].is_string());
    let mut good = server.connect_admin();
    let status = admin(&mut good, "status");
    assert_eq!(status["Status"]["protocol"], LINUX_ADMIN_PROTOCOL);
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn status_reload_and_stats_round_trip() {
    let directory = std::env::temp_dir().join(format!("qingjian-admin-rpc-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut admin_stream = server.connect_admin();
    let status = admin(&mut admin_stream, "status");
    assert!(!status["Status"]["version"].as_str().unwrap().is_empty());
    assert_eq!(status["Status"]["config"]["generation"], 0);
    assert_eq!(status["Status"]["config"]["pending"], false);
    assert!(status["Status"]["config"]["last_error"].is_null());
    // 云未接入：disabled 占位；测试资源没有模型文件，默认开启下报失败原因
    assert_eq!(status["Status"]["cloud"]["state"], "disabled");
    assert_eq!(status["Status"]["model"]["state"], "failed");

    // 真实输入一次，统计里能看到
    let mut keys = server.connect();
    open(&mut keys, 1, "admin-stats");
    key(&mut keys, 1, 'n');
    key(&mut keys, 1, 'i');
    let committed = key(&mut keys, 1, ' ');
    assert_eq!(committed["KeyResult"]["commit"], "你");
    let stats = admin(&mut admin_stream, "stats");
    assert!(
        stats["Stats"]["usage"]["today"]["commits"]
            .as_u64()
            .unwrap()
            >= 1,
        "{stats}"
    );
    assert!(stats["Stats"]["usage"]["total"]["hanzi"].as_u64().unwrap() >= 1);

    let reloaded = admin(&mut admin_stream, "reload");
    assert_eq!(reloaded["Reload"]["server"]["state"], "applied");
    assert_eq!(reloaded["Reload"]["generation"], 1);
    let status = admin(&mut admin_stream, "status");
    assert_eq!(status["Status"]["config"]["generation"], 1);
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}

mod backup;
mod maintenance;
