//! 配置热重载：不重启生效、组句不丢、坏配置保持旧配置、挂起会话也参与组句边界。
#![cfg(target_os = "linux")]
#[path = "support/server.rs"]
mod server;
use qingjian_linux_server::protocol::admin::LINUX_ADMIN_PROTOCOL;
use qingjian_platform::protocol::{PROTOCOL_VERSION, read_message, write_message};
use serde_json::{Value, json};
use server::Server;
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn admin(stream: &mut UnixStream, command: &str) -> Value {
    write_message(
        stream,
        &json!({"version": LINUX_ADMIN_PROTOCOL, "command": command}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap().unwrap()
}

fn open(stream: &mut UnixStream, context: &str) {
    write_message(
        stream,
        &json!({"OpenSession": {"session": 1, "app": "reload-test", "protocol": PROTOCOL_VERSION}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap();
    write_message(
        stream,
        &json!({"LinuxHello": {"version": 3, "session": 1, "generation": 1, "context": context}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap();
    write_message(
        stream,
        &json!({"LinuxEvent": {"session": 1, "event": {"Capabilities": {"sensitive": false, "password": false, "disabled": false}}}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap();
}

fn key(stream: &mut UnixStream, character: char) -> Value {
    write_message(
        stream,
        &json!({"LinuxEvent": {"session": 1, "event": {"Key": {"event": {"virtual_key": character as u32, "character": character, "modifiers": {"ctrl": false, "shift": false, "alt": false, "win": false, "caps": false, "english_mode": false}}, "release": false}}}}),
    )
    .unwrap();
    read_message::<_, Value>(stream).unwrap().unwrap()
}

fn page_items(frame: &Value) -> usize {
    frame["frame"]["candidates"]["items"]
        .as_array()
        .unwrap()
        .len()
}

#[test]
fn reload_applies_without_restart_and_composition_survives() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-reload-live-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut admin_stream = server.connect_admin();
    let mut keys = server.connect();
    open(&mut keys, "reload-live");
    // 缺省每页 9 个候选；样例词库对 "n" 只有 5 个
    let frame = key(&mut keys, 'n');
    assert_eq!(page_items(&frame["KeyResult"]), 5, "{frame}");
    // 组句中改配置：挂起，拼音与候选不丢
    std::fs::write(server.config_path(), "[general]\npage_size = 3\n").unwrap();
    let reloaded = admin(&mut admin_stream, "reload");
    assert_eq!(reloaded["Reload"]["server"]["state"], "pending_reload");
    let frame = key(&mut keys, 'i');
    let preedit: String = frame["KeyResult"]["frame"]["preedit"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|segment| segment["text"].as_str())
        .collect();
    assert_eq!(preedit, "ni", "{frame}");
    assert_eq!(
        frame["KeyResult"]["frame"]["candidates"]["items"][0]["text"],
        "你"
    );
    let status = admin(&mut admin_stream, "status");
    assert_eq!(status["Status"]["config"]["pending"], true);
    assert_eq!(status["Status"]["config"]["generation"], 0);
    // 继续输入并上屏，结果正确
    key(&mut keys, 'h');
    key(&mut keys, 'a');
    key(&mut keys, 'o');
    let committed = key(&mut keys, ' ');
    assert_eq!(committed["KeyResult"]["commit"], "你好");
    // 组句结束后配置生效，无需重启
    let status = admin(&mut admin_stream, "status");
    assert_eq!(status["Status"]["config"]["pending"], false);
    assert_eq!(status["Status"]["config"]["generation"], 1);
    let frame = key(&mut keys, 'n');
    assert_eq!(page_items(&frame["KeyResult"]), 3);
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn broken_config_keeps_previous_and_status_reports_error() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-reload-broken-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut admin_stream = server.connect_admin();
    std::fs::write(server.config_path(), "[general]\npage_size = 3\n").unwrap();
    assert_eq!(
        admin(&mut admin_stream, "reload")["Reload"]["server"]["state"],
        "applied"
    );
    std::fs::write(server.config_path(), "[general\n这不是 TOML\n").unwrap();
    let reloaded = admin(&mut admin_stream, "reload");
    assert_eq!(reloaded["Reload"]["server"]["state"], "failed");
    let status = admin(&mut admin_stream, "status");
    assert!(status["Status"]["config"]["last_error"].is_string());
    assert_eq!(status["Status"]["config"]["generation"], 1);
    // 旧配置仍然有效
    let mut keys = server.connect();
    open(&mut keys, "reload-broken");
    let frame = key(&mut keys, 'n');
    assert_eq!(page_items(&frame["KeyResult"]), 3);
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn mtime_poll_applies_saved_config_on_its_own() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-reload-poll-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut keys = server.connect();
    open(&mut keys, "reload-poll");
    std::fs::write(server.config_path(), "[general]\npage_size = 4\n").unwrap();
    // 轮询间隔一秒，留足余量
    std::thread::sleep(Duration::from_millis(2500));
    let frame = key(&mut keys, 'n');
    assert_eq!(page_items(&frame["KeyResult"]), 4);
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn suspended_session_delays_reload_until_its_composition_ends() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-reload-suspended-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut admin_stream = server.connect_admin();
    let mut first = server.connect();
    let mut second = server.connect();
    open(&mut first, "reload-a");
    open(&mut second, "reload-b");
    key(&mut first, 'n'); // 会话 A 组句中
    key(&mut second, 'h'); // 切到会话 B，A 挂起且组句未结束
    std::fs::write(server.config_path(), "[general]\npage_size = 3\n").unwrap();
    let reloaded = admin(&mut admin_stream, "reload");
    assert_eq!(reloaded["Reload"]["server"]["state"], "pending_reload");
    // B 的组句结束还不够，A 的组句仍挡着
    write_message(&mut second, &json!({"Commit": {"session": 1}})).unwrap();
    read_message::<_, Value>(&mut second).unwrap();
    let status = admin(&mut admin_stream, "status");
    assert_eq!(status["Status"]["config"]["pending"], true);
    // A 的组句结束（上屏挂起期间的拼音），配置生效
    write_message(&mut first, &json!({"Commit": {"session": 1}})).unwrap();
    let committed = read_message::<_, Value>(&mut first).unwrap().unwrap();
    assert_eq!(committed["Committed"]["text"], "n");
    let status = admin(&mut admin_stream, "status");
    assert_eq!(status["Status"]["config"]["pending"], false);
    let frame = key(&mut second, 'n');
    assert_eq!(page_items(&frame["KeyResult"]), 3);
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}
