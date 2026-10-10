//! 内部模式快捷键与 Fcitx5 切换分离；组合、释放及焦点边界不误切换。
use qingjian_linux_server::Router;
use qingjian_platform::protocol::{KeyEvent, KeyModifiers};
use qingjian_platform::{SwitchKey, SwitchKeys};
use serde_json::{Value, json};

use super::support::{compose, event, key, router_with_switch};

fn chord(router: &mut Router, release: bool, modifiers: KeyModifiers) -> Value {
    event(
        router,
        1,
        json!({"Key": {"event": KeyEvent::new(32, Some(' '), modifiers), "release": release}}),
    )
}

fn ctrl_alt() -> KeyModifiers {
    KeyModifiers {
        ctrl: true,
        alt: true,
        ..Default::default()
    }
}

fn assert_chinese(router: &mut Router) {
    compose(router, 1, "nihao");
    assert_eq!(key(router, 1, 32, Some(' '), false)["commit"], "你好");
}

fn assert_english(router: &mut Router) {
    assert_eq!(
        compose(router, 1, "hello")["frame"]["candidates"]["items"][0]["text"],
        "hello"
    );
    assert_eq!(key(router, 1, 13, None, false)["commit"], "hello");
}

#[test]
fn chord_switches_once_commits_pending_input_and_leaves_shift_to_fcitx() {
    let mut router = router_with_switch(SwitchKeys::NONE.with(SwitchKey::CtrlAltSpace, true));
    key(&mut router, 1, 16, None, false);
    assert_eq!(
        key(&mut router, 1, 16, None, true)["outcome"],
        "Passthrough"
    );
    assert_chinese(&mut router);
    compose(&mut router, 1, "ni");
    let switched = chord(&mut router, false, ctrl_alt());
    assert_eq!(switched["commit"], "ni");
    assert_eq!(switched["outcome"], "Consumed");
    assert_eq!(chord(&mut router, true, ctrl_alt())["commit"], json!(null));
    assert_english(&mut router);
    chord(&mut router, false, ctrl_alt());
    assert_chinese(&mut router);
}

#[test]
fn disabled_or_extra_modified_chords_do_not_switch_or_commit() {
    let mut router = router_with_switch(SwitchKeys::NONE);
    compose(&mut router, 1, "ni");
    assert_eq!(
        chord(&mut router, false, ctrl_alt())["outcome"],
        "Passthrough"
    );
    assert_eq!(key(&mut router, 1, 13, None, false)["commit"], "ni");
    let mut router = router_with_switch(SwitchKeys::NONE.with(SwitchKey::CtrlAltSpace, true));
    for modifiers in [
        KeyModifiers {
            shift: true,
            ..ctrl_alt()
        },
        KeyModifiers {
            win: true,
            ..ctrl_alt()
        },
    ] {
        assert_eq!(
            chord(&mut router, false, modifiers)["outcome"],
            "Passthrough"
        );
        assert_chinese(&mut router);
    }
}

#[test]
fn control_tap_switches_but_control_combinations_do_not() {
    let mut router = router_with_switch(SwitchKeys::NONE.with(SwitchKey::Control, true));
    key(&mut router, 1, 17, None, false);
    event(
        &mut router,
        1,
        json!({"Key": {"event": KeyEvent::new('c' as u32, Some('c'), KeyModifiers {
            ctrl: true, ..Default::default()
        }), "release": false}}),
    );
    assert_eq!(
        key(&mut router, 1, 17, None, true)["outcome"],
        "Passthrough"
    );
    assert_chinese(&mut router);
    key(&mut router, 1, 17, None, false);
    assert_eq!(key(&mut router, 1, 17, None, true)["outcome"], "Consumed");
    assert_english(&mut router);
}

#[test]
fn focus_changes_cancel_a_pending_modifier_tap() {
    let mut router = router_with_switch(SwitchKeys::NONE.with(SwitchKey::Control, true));
    key(&mut router, 1, 17, None, false);
    event(&mut router, 1, json!({"Focus": {"focused": false}}));
    event(&mut router, 1, json!({"Focus": {"focused": true}}));
    assert_eq!(
        key(&mut router, 1, 17, None, true)["outcome"],
        "Passthrough"
    );
    assert_chinese(&mut router);
}
