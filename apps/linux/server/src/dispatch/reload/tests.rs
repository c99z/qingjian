//! 热重载的组句边界与失败保持：直接驱动 Router，不过 socket。
use super::ReloadDirs;
use crate::dispatch::{Router, RouterConfig};
use crate::protocol::admin::AdminCommand;
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::Config;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use qingjian_platform::settings::ComponentStatus;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn dirs(root: &Path) -> ReloadDirs {
    ReloadDirs {
        resource_root: root.to_owned(),
        user_dir: None,
        bundled_dicts: None,
        input_log: None,
    }
}

fn workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("qingjian-reload-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn router(config_path: &Path, root: &Path) -> Router {
    let config = Config::load(config_path).unwrap();
    let mut router = Router::new(
        Engine::new(Dictionary::parse("你\tni\t100\n好\thao\t90\n").unwrap()),
        RouterConfig::from(&config),
    );
    router.watch_config(&config, config_path.to_owned(), dirs(root));
    router
}

fn key(router: &mut Router, session: u64, c: char) {
    router.handle(ClientMessage::Key {
        session: SessionId(session),
        event: KeyEvent::new(c as u32, Some(c), KeyModifiers::default()),
    });
}

fn open(router: &mut Router, session: u64) {
    router.handle(ClientMessage::OpenSession {
        session: SessionId(session),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
}

fn reload(router: &mut Router) -> ComponentStatus {
    let value = router
        .handle_admin(serde_json::json!({"Admin": AdminCommand::Reload}))
        .expect("reload response");
    serde_json::from_value(value["Reload"]["server"].clone()).unwrap()
}

#[test]
fn idle_reload_applies_immediately_and_bumps_generation() {
    let root = workspace("idle");
    let path = root.join("config.toml");
    std::fs::write(&path, "[general]\n").unwrap();
    let mut router = router(&path, &root);
    assert_eq!(router.config.page_size, 9);
    std::fs::write(&path, "[general]\npage_size = 3\n").unwrap();
    assert_eq!(reload(&mut router), ComponentStatus::Applied);
    assert_eq!(router.config.page_size, 3);
    assert_eq!(router.reload_generation(), 1);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn composing_session_defers_until_boundary_and_keeps_input() {
    let root = workspace("defer");
    let path = root.join("config.toml");
    std::fs::write(&path, "[general]\n").unwrap();
    let mut router = router(&path, &root);
    open(&mut router, 1);
    key(&mut router, 1, 'n');
    assert!(router.any_composing());
    std::fs::write(&path, "[general]\npage_size = 3\n").unwrap();
    assert_eq!(reload(&mut router), ComponentStatus::PendingReload);
    assert_eq!(router.config.page_size, 9);
    assert_eq!(router.reload_generation(), 0);
    // 组句继续，拼音还在
    key(&mut router, 1, 'i');
    assert_eq!(router.engine.composition().text(), "ni");
    router.handle(ClientMessage::Commit {
        session: SessionId(1),
    });
    router.tick();
    assert_eq!(router.config.page_size, 3);
    assert_eq!(router.reload_generation(), 1);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn suspended_session_also_blocks_application() {
    let root = workspace("suspend");
    let path = root.join("config.toml");
    std::fs::write(&path, "[general]\n").unwrap();
    let mut router = router(&path, &root);
    open(&mut router, 1);
    key(&mut router, 1, 'n');
    open(&mut router, 2);
    key(&mut router, 2, 'h'); // 会话 1 挂起且仍在组句
    std::fs::write(&path, "[general]\npage_size = 3\n").unwrap();
    assert_eq!(reload(&mut router), ComponentStatus::PendingReload);
    // 会话 2 组句结束还不够，会话 1 的组句也挂着
    router.handle(ClientMessage::Commit {
        session: SessionId(2),
    });
    router.maybe_apply_pending();
    assert_eq!(router.config.page_size, 9);
    router.handle(ClientMessage::Commit {
        session: SessionId(1),
    });
    router.maybe_apply_pending();
    assert_eq!(router.config.page_size, 3);
    // 挂起期间会话 1 的拼音没丢
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn broken_config_keeps_the_old_one_and_reports_the_error() {
    let root = workspace("broken");
    let path = root.join("config.toml");
    std::fs::write(&path, "[general]\npage_size = 3\n").unwrap();
    let mut router = router(&path, &root);
    assert_eq!(router.config.page_size, 3);
    std::fs::write(&path, "[general\n这不是 TOML\n").unwrap();
    let status = reload(&mut router);
    assert!(matches!(status, ComponentStatus::Failed(_)), "{status:?}");
    assert_eq!(router.config.page_size, 3);
    assert_eq!(router.reload_generation(), 0);
    let status = router.handle_admin(serde_json::json!({"Admin": AdminCommand::Status}));
    let last_error = status.unwrap()["Status"]["config"]["last_error"].clone();
    assert!(last_error.is_string(), "{last_error}");
    // 修好后下一次重载正常应用
    std::fs::write(&path, "[general]\npage_size = 5\n").unwrap();
    assert_eq!(reload(&mut router), ComponentStatus::Applied);
    assert_eq!(router.config.page_size, 5);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn mtime_poll_picks_up_changes_after_the_interval() {
    let root = workspace("poll");
    let path = root.join("config.toml");
    std::fs::write(&path, "[general]\n").unwrap();
    let mut router = router(&path, &root);
    std::fs::write(&path, "[general]\npage_size = 4\n").unwrap();
    router.poll_config_reload(); // 一秒节流内不真看
    assert_eq!(router.config.page_size, 9);
    router.reload.as_mut().unwrap().last_check = Instant::now() - Duration::from_secs(2);
    router.poll_config_reload();
    assert_eq!(router.config.page_size, 4);
    std::fs::remove_dir_all(&root).unwrap();
}
