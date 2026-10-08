//! 保存管线的决策：校验不过不写、冲突不写、服务没跑记「下次生效」、重载失败如实报部分成功。

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;

use qingjian_platform::protocol::linux_admin::{AdminRequest, AdminResponse, ReloadReport};
use qingjian_platform::protocol::{read_message, write_message};
use qingjian_platform::settings::ComponentStatus;

use super::{SaveOutcome, save};
use crate::adapters::admin::AdminClient;
use crate::state::DraftStore;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qingjian-save-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn temp_config(name: &str, content: &str) -> PathBuf {
    let path = temp_dir().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

/// 回一个固定的 Reload 结果的假服务。
fn fake_reload_server(
    name: &str,
    server_status: ComponentStatus,
) -> (AdminClient, thread::JoinHandle<()>) {
    let path = temp_dir().join(name);
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    let cleanup = path.clone();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request: AdminRequest = read_message(&mut stream).unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(request.command).unwrap(),
            serde_json::Value::String("reload".to_owned())
        );
        write_message(
            &mut stream,
            &AdminResponse::Reload(ReloadReport {
                generation: 1,
                server: server_status,
            }),
        )
        .unwrap();
        let _ = std::fs::remove_file(&cleanup);
    });
    (AdminClient::with_path(path), handle)
}

#[test]
fn a_valid_draft_is_written_and_reloaded() {
    let path = temp_config("ok.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 5i64);
    let (admin, server) = fake_reload_server("ok.sock", ComponentStatus::Applied);
    let outcome = save(store.path(), store.snapshot(), store.patch(), &admin, None);
    assert!(matches!(
        outcome,
        SaveOutcome::Saved {
            server: ComponentStatus::Applied,
            ..
        }
    ));
    server.join().unwrap();
    store.mark_saved(true).unwrap();
    assert!(!store.is_dirty());
    assert_eq!(store.config().general.page_size, 5);
}

#[test]
fn validation_errors_write_nothing() {
    let path = temp_config("invalid.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 42i64);
    let admin = AdminClient::with_path(temp_dir().join("invalid.sock"));
    let outcome = save(store.path(), store.snapshot(), store.patch(), &admin, None);
    let SaveOutcome::Invalid(errors) = outcome else {
        panic!("应当校验不过：{outcome:?}");
    };
    assert_eq!(errors[0].field, "general.page_size");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[general]\npage_size = 9\n",
        "校验不过时文件不动"
    );
}

#[test]
fn conflicts_write_nothing_and_list_the_fields() {
    let path = temp_config("conflict.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 5i64);
    // 编辑期间外部把同一字段改成了别的值
    std::fs::write(&path, "[general]\npage_size = 7\n").unwrap();
    let admin = AdminClient::with_path(temp_dir().join("conflict.sock"));
    let outcome = save(store.path(), store.snapshot(), store.patch(), &admin, None);
    let SaveOutcome::Conflicts(conflicts) = outcome else {
        panic!("应当报冲突：{outcome:?}");
    };
    assert_eq!(conflicts[0].field, "general.page_size");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[general]\npage_size = 7\n"
    );
}

#[test]
fn saving_without_the_server_reports_saved_for_next_start() {
    let path = temp_config("offline.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("model", "enabled", false);
    let missing = temp_dir().join("offline.sock");
    let _ = std::fs::remove_file(&missing);
    let admin = AdminClient::with_path(missing);
    let outcome = save(store.path(), store.snapshot(), store.patch(), &admin, None);
    assert!(
        matches!(
            outcome,
            SaveOutcome::Saved {
                server: ComponentStatus::Saved,
                ..
            }
        ),
        "服务未运行也应保存成功并记 Saved：{outcome:?}"
    );
    assert!(
        !qingjian_platform::Config::load(&path)
            .unwrap()
            .model
            .enabled
    );
}

#[test]
fn a_failed_reload_is_reported_as_partial_success() {
    let path = temp_config("reload-fails.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 4i64);
    let (admin, server) = fake_reload_server(
        "reload-fails.sock",
        ComponentStatus::Failed("配置解析失败".to_owned()),
    );
    let outcome = save(store.path(), store.snapshot(), store.patch(), &admin, None);
    server.join().unwrap();
    match outcome {
        SaveOutcome::Saved {
            server: ComponentStatus::Failed(message),
            ..
        } => assert!(message.contains("配置解析失败")),
        other => panic!("重载失败要如实报出来：{other:?}"),
    }
    // 配置已经落盘：部分成功，不许显示成全部成功
    assert_eq!(
        qingjian_platform::Config::load(&path)
            .unwrap()
            .general
            .page_size,
        4
    );
}

#[test]
fn a_broken_current_file_refuses_the_write() {
    let path = temp_config("broken.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 5i64);
    // 读完快照后文件被外部改坏
    std::fs::write(&path, "[general\npage_size = 9\n").unwrap();
    let admin = AdminClient::with_path(temp_dir().join("broken.sock"));
    let outcome = save(store.path(), store.snapshot(), store.patch(), &admin, None);
    let SaveOutcome::WriteFailed(message) = outcome else {
        panic!("坏文件不应写入：{outcome:?}");
    };
    assert!(message.contains("语法错误"), "{message}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[general\npage_size = 9\n"
    );
}

#[test]
fn appearance_failure_keeps_the_draft_and_does_not_claim_full_success() {
    let path = temp_config("appearance.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    // Fcitx5 没跑（基准为空）时改了主题：配置照存，外观报失败并保留草稿
    store.appearance_mut().set_theme(Some("default".to_owned()));
    store.set("general", "page_size", 6i64);
    let admin = AdminClient::with_path(temp_dir().join("appearance.sock"));
    let appearance = store
        .appearance()
        .is_dirty()
        .then(|| store.appearance().clone());
    let outcome = save(
        store.path(),
        store.snapshot(),
        store.patch(),
        &admin,
        appearance.as_ref(),
    );
    match outcome {
        SaveOutcome::Saved {
            server: ComponentStatus::Saved,
            appearance: Some(ComponentStatus::Failed(message)),
        } => assert!(message.contains("保留在草稿中"), "{message}"),
        other => panic!("外观未应用要如实报出来：{other:?}"),
    }
    // 配置部分照常落盘
    assert_eq!(
        qingjian_platform::Config::load(&path)
            .unwrap()
            .general
            .page_size,
        6
    );
    // 外观没应用：基准推进时保留外观草稿
    store.mark_saved(false).unwrap();
    assert!(store.is_dirty(), "外观草稿要留着等重试");
    store.mark_saved(true).unwrap();
    assert!(!store.is_dirty());
}
