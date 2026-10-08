//! 空格译词快捷键：普通空格选中文，高亮 / 分页、缺释义和修改修饰键的实际输出。
use super::support::{compose, key, router};
use qingjian_core::{Engine, Language};
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{KeyModifiers, KeyOutcome};
use qingjian_translate::Glossary;

const SHIFT: KeyModifiers = KeyModifiers {
    shift: true,
    ctrl: false,
    alt: false,
    win: false,
    caps: false,
    english_mode: false,
};

#[test]
fn space_selects_chinese_and_shift_space_selects_first_translation() {
    let mut router = router(5);
    router.engine.set_translator(Box::new(
        Glossary::parse(Language::English, "你\tpron. you\tthou\n").unwrap(),
    ));
    let normal = KeyModifiers::default();
    compose(&mut router, "ni", normal);
    assert_eq!(
        key(&mut router, 32, Some(' '), normal).1.as_deref(),
        Some("你")
    );
    compose(&mut router, "ni", normal);
    let result = key(&mut router, 32, Some(' '), SHIFT);
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert_eq!(result.1.as_deref(), Some("you"));
    assert!(result.2.is_empty());
    assert!(!router.engine.english_mode());
}

#[test]
fn shortcut_uses_highlight_on_the_current_page() {
    let mut router = router(2);
    router.engine = Engine::new(
        Dictionary::parse("你\tni\t100\n泥\tni\t90\n拟\tni\t80\n倪\tni\t70\n").unwrap(),
    )
    .with_translator(Box::new(
        Glossary::parse(Language::English, "倪\tNi\n").unwrap(),
    ));
    let normal = KeyModifiers::default();
    compose(&mut router, "ni", normal);
    key(&mut router, b']' as u32, Some(']'), normal);
    let frame = key(&mut router, 0x28, None, normal).2;
    assert_eq!((frame.page, frame.highlight), (1, 1));
    assert_eq!(
        key(&mut router, 32, Some(' '), SHIFT).1.as_deref(),
        Some("Ni")
    );
}

#[test]
fn missing_translation_keeps_composition_and_empty_composition_passes_through() {
    let mut router = router(5);
    assert_eq!(
        key(&mut router, 32, Some(' '), SHIFT).0,
        KeyOutcome::Passthrough
    );
    compose(&mut router, "ni", KeyModifiers::default());
    let before = router.engine.composition().text().to_owned();
    let result = key(&mut router, 32, Some(' '), SHIFT);
    assert_eq!(result.0, KeyOutcome::Consumed);
    assert!(result.1.is_none());
    assert_eq!(router.engine.composition().text(), before);
    assert_eq!(result.2.candidates.items[0].text, "你");
}

#[test]
fn configured_modifier_is_used_and_raw_or_english_space_keeps_its_behavior() {
    let mut router = router(5);
    router.engine.set_translator(Box::new(
        Glossary::parse(Language::English, "你\tyou\n").unwrap(),
    ));
    let alt = KeyModifiers {
        alt: true,
        ..Default::default()
    };
    router.config.translation_space_keys = alt;
    compose(&mut router, "ni", KeyModifiers::default());
    assert_eq!(
        key(&mut router, 32, Some(' '), alt).1.as_deref(),
        Some("you")
    );
    compose(&mut router, "ni", KeyModifiers::default());
    assert_eq!(
        key(&mut router, 32, Some(' '), SHIFT).1.as_deref(),
        Some("你")
    );
    router.config.translation_space_keys = SHIFT;
    compose(&mut router, "gpt-6", KeyModifiers::default());
    assert_eq!(
        key(&mut router, 32, Some(' '), SHIFT).1.as_deref(),
        Some("gpt-6 ")
    );
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    compose(&mut router, "hel", english);
    let english_shift = KeyModifiers {
        shift: true,
        ..english
    };
    assert_eq!(
        key(&mut router, 32, Some(' '), english_shift).1.as_deref(),
        Some("hello ")
    );
}
