//! 服务中最新统计进入备份，恢复前拒绝未上屏输入，恢复后内存与磁盘一致。
use super::{Server, admin, key, open};
use qingjian_platform::{
    backup::Archive,
    protocol::linux_admin::{AdminCommand, AdminRequest, LINUX_ADMIN_PROTOCOL},
    protocol::{read_message, write_message},
};
use serde_json::Value;
#[test]
fn backup_restore_preserves_live_data_and_refuses_active_composition() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-admin-backup-{}", std::process::id()));
    let mut server = Server::start(directory.clone());
    let mut control = server.connect_admin();
    std::fs::write(
        server.config_path(),
        "[general]\nlearning_language = 'off'\n[model]\nenabled = false\n",
    )
    .unwrap();
    admin(&mut control, "reload");
    let mut keys = server.connect();
    open(&mut keys, 1, "backup-test");
    key(&mut keys, 1, 'n');
    key(&mut keys, 1, 'i');
    key(&mut keys, 1, ' ');
    let backup = admin(&mut control, "backup");
    let file = backup["Backup"].as_str().unwrap();
    let archive = Archive::read(std::path::Path::new(file)).unwrap();
    assert!(archive.files.contains_key("data/usage.tsv"));
    assert_eq!(
        admin(&mut control, "stats")["Stats"]["usage"]["total"]["commits"],
        1
    );
    key(&mut keys, 1, 'n');
    key(&mut keys, 1, 'i');
    key(&mut keys, 1, ' ');
    let transfer = "e88d386c-118d-4d82-9cdd-3d8da6866d61";
    let input = directory.join(format!("data/qingjian/transfers/{transfer}.qjb"));
    archive.write(&input).unwrap();
    key(&mut keys, 1, 'n');
    let restore = |stream: &mut std::os::unix::net::UnixStream| {
        write_message(
            stream,
            &AdminRequest {
                version: LINUX_ADMIN_PROTOCOL,
                command: AdminCommand::Restore {
                    transfer: transfer.into(),
                },
            },
        )
        .unwrap();
        read_message::<_, Value>(stream).unwrap().unwrap()
    };
    assert_eq!(restore(&mut control)["Maintenance"]["state"], "failed");
    key(&mut keys, 1, 'i');
    assert_eq!(key(&mut keys, 1, ' ')["KeyResult"]["commit"], "你");
    assert_eq!(restore(&mut control)["Maintenance"]["state"], "applied");
    assert_eq!(
        admin(&mut control, "stats")["Stats"]["usage"]["total"]["commits"],
        1
    );
    key(&mut keys, 1, 'n');
    key(&mut keys, 1, 'i');
    assert_eq!(key(&mut keys, 1, ' ')["KeyResult"]["commit"], "你");
    assert_eq!(
        admin(&mut control, "stats")["Stats"]["usage"]["total"]["commits"],
        2
    );
    assert!(
        std::fs::read_dir(directory.join("data/qingjian/backups"))
            .unwrap()
            .count()
            >= 2
    );
    drop(server);
    std::fs::remove_dir_all(directory).unwrap();
}
