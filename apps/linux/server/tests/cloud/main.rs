//! 真服务对本机 AI 接口：候选、Tab 补全、默认无上下文、失焦和关闭时旧结果作废。
mod mock;
#[path = "../support/server.rs"]
mod server;
use mock::MockAi;
use qingjian_platform::protocol::{
    PROTOCOL_VERSION, linux_admin::LINUX_ADMIN_PROTOCOL, read_message, write_message,
};
use serde_json::{Value, json};
use server::Server;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
fn request(stream: &mut UnixStream, value: Value) -> Value {
    write_message(stream, &value).unwrap();
    read_message::<_, Value>(stream).unwrap().unwrap()
}
fn admin(stream: &mut UnixStream, command: &str) -> Value {
    request(
        stream,
        json!({"version":LINUX_ADMIN_PROTOCOL,"command":command}),
    )
}
fn key(stream: &mut UnixStream, character: char) -> Value {
    request(
        stream,
        json!({"LinuxEvent":{"session":1,"event":{"Key":{"event":{"virtual_key":character as u32,"character":character,"modifiers":{"ctrl":false,"shift":false,"alt":false,"win":false,"caps":false,"english_mode":false}},"release":false}}}}),
    )
}
fn open(stream: &mut UnixStream) {
    request(
        stream,
        json!({"OpenSession":{"session":1,"app":"mock-ai-test","protocol":PROTOCOL_VERSION}}),
    );
    request(
        stream,
        json!({"LinuxHello":{"version":3,"session":1,"generation":1,"context":"mock"}}),
    );
    request(
        stream,
        json!({"LinuxEvent":{"session":1,"event":{"Capabilities":{"sensitive":false,"password":false,"disabled":false}}}}),
    );
}
fn poll(stream: &mut UnixStream) -> Value {
    request(stream, json!({"Poll":{"session":1}}))["Update"]["frame"].clone()
}
fn wait_until(mut check: impl FnMut() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(start.elapsed() < Duration::from_secs(5), "等待云结果超时");
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn cloud_predicts_without_surrounding_text_and_accepts_tab() {
    let mock = MockAi::start();
    let directory =
        std::env::temp_dir().join(format!("qingjian-cloud-input-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut control = server.connect_admin();
    let enabled = format!(
        "[general]\nlearning_language='off'\n[model]\nenabled=false\n[predict]\nenabled=true\nbase_url='{}'\nmodel='mock'\napi_key='mock-only'\ndebounce_ms=100\n",
        mock.url
    );
    std::fs::write(server.config_path(), &enabled).unwrap();
    admin(&mut control, "reload");
    assert_eq!(
        admin(&mut control, "status")["Status"]["cloud"]["state"],
        "ready"
    );
    let mut keys = server.connect();
    open(&mut keys);
    for c in "nihao".chars() {
        key(&mut keys, c);
    }
    let mut frame = Value::Null;
    wait_until(|| {
        frame = poll(&mut keys);
        frame["sentence"] == "你好，世界。"
    });
    assert!(
        frame["candidates"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["text"] == "拟好"),
        "{frame}"
    );
    let captured = mock.requests.lock().unwrap();
    let body = captured.last().unwrap();
    let prompt: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(prompt["before"], "");
    assert_eq!(prompt["after"], "");
    drop(captured);
    assert_eq!(key(&mut keys, '\t')["KeyResult"]["commit"], "你好，世界。");
    // 换模型重建请求线程，避免第二次输入直接命中首轮缓存。
    std::fs::write(
        server.config_path(),
        enabled.replace("model='mock'", "model='mock-second'"),
    )
    .unwrap();
    admin(&mut control, "reload");
    // 已发出的请求在失焦后不许回到当前面板。
    let previous = mock.requests.lock().unwrap().len();
    for c in "nihao".chars() {
        key(&mut keys, c);
    }
    wait_until(|| mock.requests.lock().unwrap().len() > previous);
    request(
        &mut keys,
        json!({"LinuxEvent":{"session":1,"event":{"Focus":{"focused":false}}}}),
    );
    std::thread::sleep(Duration::from_millis(180));
    assert!(poll(&mut keys)["sentence"].is_null());
    request(
        &mut keys,
        json!({"LinuxEvent":{"session":1,"event":{"Focus":{"focused":true}}}}),
    );
    // 关闭云服务立即生效，即使配置其他部分等组句结束。
    std::fs::write(
        server.config_path(),
        enabled.replace("enabled=true", "enabled=false"),
    )
    .unwrap();
    admin(&mut control, "reload");
    assert_eq!(
        admin(&mut control, "status")["Status"]["cloud"]["state"],
        "disabled"
    );
    assert!(poll(&mut keys)["sentence"].is_null());
    assert_eq!(key(&mut keys, ' ')["KeyResult"]["commit"], "你好");
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}
