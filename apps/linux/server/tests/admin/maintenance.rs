//! 服务协调日志清空，不丢输入和统计、不重新写回旧缓冲。

use super::{Server, admin, key, open};

#[test]
fn clearing_buffered_logs_preserves_composition_and_usage() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-admin-clear-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut control = server.connect_admin();
    std::fs::write(
        server.config_path(),
        "[general]\ninput_log = true\n[model]\nenabled = false\n",
    )
    .unwrap();
    assert_eq!(
        admin(&mut control, "reload")["Reload"]["server"]["state"],
        "applied"
    );
    let mut keys = server.connect();
    open(&mut keys, 1, "clear-log-test");
    key(&mut keys, 1, 'n');
    key(&mut keys, 1, 'i');
    assert_eq!(key(&mut keys, 1, ' ')["KeyResult"]["commit"], "你");
    key(&mut keys, 1, 'n');
    assert_eq!(
        admin(&mut control, "clear_input_log")["Maintenance"]["state"],
        "applied"
    );
    let path = directory.join("state/qingjian/logs/input-log.jsonl");
    assert!(std::fs::read(&path).unwrap().is_empty());
    key(&mut keys, 1, 'i');
    assert_eq!(
        key(&mut keys, 1, ' ')["KeyResult"]["commit"],
        "你",
        "清空日志不能丢组句"
    );
    // 关闭写入端使新记录落盘；旧缓冲不得重新出现，只有清空后的这次提交。
    std::fs::write(
        server.config_path(),
        "[general]\ninput_log = false\n[model]\nenabled = false\n",
    )
    .unwrap();
    admin(&mut control, "reload");
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    let stats = admin(&mut control, "stats");
    assert_eq!(stats["Stats"]["usage"]["total"]["commits"], 2);
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}
