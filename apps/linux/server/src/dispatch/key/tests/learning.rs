//! 通过 Linux 按下 / 释放事件验证退格后的学习撤销，不能直接调用 Engine 删除通知。

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_learning::FrequencyLearner;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use serde_json::{Value, json};

use crate::dispatch::{Router, RouterConfig};

fn router() -> Router {
    let engine = Engine::new(
        Dictionary::parse("再\tzai\t100\n在\tzai\t100\n开发\tkai fa\t100\n开法\tkai fa\t90\n")
            .unwrap(),
    )
    .with_learner(Box::new(FrequencyLearner::default()));
    let mut router = Router::new(engine, RouterConfig::default());
    for id in [1, 2] {
        router.handle(ClientMessage::OpenSession {
            session: SessionId(id),
            app: None,
            protocol: PROTOCOL_VERSION,
        });
        router.handle_linux(json!({"LinuxEvent":{"session":id,"event":{"Capabilities":{"sensitive":false,"password":false,"disabled":false}}}}));
    }
    router
}

fn key(
    router: &mut Router,
    id: u64,
    code: u32,
    character: Option<char>,
    modifiers: KeyModifiers,
    release: bool,
) -> Value {
    router
        .handle_linux(json!({"LinuxEvent":{"session":id,"event":{"Key":{
            "event":KeyEvent::new(code, character, modifiers),"release":release
        }}}}))
        .unwrap()["KeyResult"]
        .clone()
}

fn press(router: &mut Router, id: u64, code: u32) -> Value {
    key(router, id, code, None, KeyModifiers::default(), false)
}

fn pick(router: &mut Router, id: u64, input: &str, text: &str) -> Value {
    let mut result = Value::Null;
    for c in input.chars() {
        result = key(
            router,
            id,
            c as u32,
            Some(c),
            KeyModifiers::default(),
            false,
        );
    }
    let index = result["frame"]["candidates"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| c["text"] == text)
        .unwrap();
    let digit = char::from(b'1' + index as u8);
    let result = key(
        router,
        id,
        digit as u32,
        Some(digit),
        KeyModifiers::default(),
        false,
    );
    assert_eq!(result["commit"], text);
    result
}

#[test]
fn deleting_and_reselecting_retracts_counts_recent_choices_and_transitions() {
    let mut router = router();
    pick(&mut router, 1, "zai", "再");
    assert_eq!(router.engine.learner().choice_weight("zai", "再"), 1);
    let back = press(&mut router, 1, 8);
    assert_eq!(back["outcome"], "Passthrough");
    assert!(back["commit"].is_null());
    key(&mut router, 1, 8, None, KeyModifiers::default(), true);
    pick(&mut router, 1, "zai", "在");
    let learner = router.engine.learner();
    assert_eq!(learner.weight("再"), 0);
    assert_eq!(learner.choice_weight("zai", "再"), 0);
    assert_eq!(learner.recent_choice_weight("zai", "再"), 0.0);
    assert_eq!(learner.choice_weight("zai", "在"), 1);
    assert_eq!(learner.recent_choice_weight("zai", "在"), 1.0);
    assert_eq!(learner.user_ngram().unwrap().pair(Some("再"), "在"), 0);
}

#[test]
fn deleting_a_whole_word_counts_presses_without_counting_releases() {
    let mut router = router();
    pick(&mut router, 1, "kaifa", "开发");
    for _ in 0..2 {
        assert_eq!(press(&mut router, 1, 8)["outcome"], "Passthrough");
        key(&mut router, 1, 8, None, KeyModifiers::default(), true);
    }
    pick(&mut router, 1, "kaifa", "开法");
    assert_eq!(router.engine.learner().choice_weight("kaifa", "开发"), 0);
    assert_eq!(router.engine.learner().choice_weight("kaifa", "开法"), 1);
}

#[test]
fn a_partial_deletion_or_backspace_inside_preedit_does_not_retract_a_word() {
    let mut router = router();
    pick(&mut router, 1, "kaifa", "开发");
    press(&mut router, 1, 8);
    key(&mut router, 1, 8, None, KeyModifiers::default(), true);
    pick(&mut router, 1, "kaifa", "开法");
    assert_eq!(router.engine.learner().choice_weight("kaifa", "开发"), 1);
    pick(&mut router, 1, "zai", "再");
    key(
        &mut router,
        1,
        b'z' as u32,
        Some('z'),
        KeyModifiers::default(),
        false,
    );
    assert_eq!(press(&mut router, 1, 8)["outcome"], "Consumed");
    pick(&mut router, 1, "zai", "在");
    assert_eq!(router.engine.learner().choice_weight("zai", "再"), 1);
}

#[test]
fn navigation_and_modified_deletion_end_ambiguous_correction_tracking() {
    for code in [0x25, 0x26, 0x27, 0x28, 0x24, 0x23, 0x21, 0x22, 0x09, 0x2e] {
        let mut router = router();
        pick(&mut router, 1, "zai", "再");
        assert_eq!(press(&mut router, 1, code)["outcome"], "Passthrough");
        press(&mut router, 1, 8);
        pick(&mut router, 1, "zai", "在");
        assert_eq!(router.engine.learner().choice_weight("zai", "再"), 1);
    }
    let mut router = router();
    pick(&mut router, 1, "zai", "再");
    let ctrl = KeyModifiers {
        ctrl: true,
        ..Default::default()
    };
    assert_eq!(
        key(&mut router, 1, 8, None, ctrl, false)["outcome"],
        "Passthrough"
    );
    press(&mut router, 1, 8);
    pick(&mut router, 1, "zai", "在");
    assert_eq!(router.engine.learner().choice_weight("zai", "再"), 1);
}

#[test]
fn a_backspace_in_another_context_cannot_retract_the_first_context() {
    let mut router = router();
    pick(&mut router, 1, "zai", "再");
    press(&mut router, 2, 8);
    pick(&mut router, 1, "zai", "在");
    assert_eq!(router.engine.learner().choice_weight("zai", "再"), 1);
}

#[test]
fn privacy_and_disabled_learning_do_not_write_corrections() {
    for private in [false, true] {
        let mut router = router();
        if private {
            router.handle_linux(json!({"LinuxEvent":{"session":1,"event":{"Capabilities":{"sensitive":true,"password":false,"disabled":false}}}}));
        } else {
            router.engine.set_learning(false);
        }
        pick(&mut router, 1, "zai", "再");
        press(&mut router, 1, 8);
        pick(&mut router, 1, "zai", "在");
        assert_eq!(router.engine.learner().choice_weight("zai", "再"), 0);
        assert_eq!(router.engine.learner().choice_weight("zai", "在"), 0);
    }
}
