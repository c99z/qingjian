//! 热重载保持旧设置直到所有组句结束，失败时保持可用配置。

use super::*;

#[test]
fn switching_shortcuts_reload_and_cancel_pending_taps() {
    use qingjian_platform::{SwitchKey, SwitchKeys};

    let root = workspace("switching");
    let path = root.join("config.toml");
    std::fs::write(&path, "[shortcut]\nswitch_mode = [\"shift\"]\n").unwrap();
    let mut router = router(&path, &root);
    open(&mut router, 1);
    assert!(!router.mode_switch_key(
        SessionId(1),
        &KeyEvent::new(16, None, KeyModifiers::default()),
        false,
    ));
    std::fs::write(&path, "[shortcut]\nswitch_mode = [\"ctrl+alt+space\"]\n").unwrap();
    assert_eq!(reload(&mut router), ComponentStatus::Applied);
    assert_eq!(
        router.config.switch_mode,
        SwitchKeys::NONE.with(SwitchKey::CtrlAltSpace, true)
    );
    assert!(!router.mode_switch_key(
        SessionId(1),
        &KeyEvent::new(16, None, KeyModifiers::default()),
        true,
    ));
    assert!(router.mode_switch_key(
        SessionId(1),
        &KeyEvent::new(
            32,
            Some(' '),
            KeyModifiers {
                ctrl: true,
                alt: true,
                ..Default::default()
            }
        ),
        false,
    ));
    let _ = std::fs::remove_dir_all(root);
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
    std::fs::write(
        &path,
        "[general]\npage_size = 3\nchinese_input_style = \"chinese\"\n",
    )
    .unwrap();
    assert_eq!(reload(&mut router), ComponentStatus::PendingReload);
    assert_eq!(router.config.page_size, 9);
    assert_eq!(router.reload_generation(), 0);
    assert!(router.engine.chinese_input_style().mixed());
    // 组句继续，拼音还在
    key(&mut router, 1, 'i');
    assert_eq!(router.engine.composition().text(), "ni");
    router.handle(ClientMessage::Commit {
        session: SessionId(1),
    });
    router.tick();
    assert_eq!(router.config.page_size, 3);
    assert_eq!(router.reload_generation(), 1);
    assert!(!router.engine.chinese_input_style().mixed());
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
