//! 外观写入计划、冲突检测与读回确认。
use super::*;
use crate::adapters::fcitx5::font::FontWeight;
use crate::adapters::fcitx5::theme::CommentStyle;

fn base() -> ClassicUiState {
    ClassicUiState {
        font: FontDesc::parse("Sans 10"),
        theme: "default".to_owned(),
        vertical: false,
        follow_system_theme: false,
        themes: vec![],
        comment: CommentStyle::default(),
    }
}

#[test]
fn identical_values_plan_nothing() {
    let values = AppearanceValues {
        font: Some(FontDesc::parse("Sans 10")),
        theme: Some("default".to_owned()),
        ..AppearanceValues::default()
    };
    assert!(plan(&base(), &values).is_empty());
}

#[test]
fn font_and_theme_changes_map_to_classicui_keys() {
    let values = AppearanceValues {
        font: Some(FontDesc {
            family: "Noto Sans CJK SC".to_owned(),
            weight: FontWeight::Bold,
            size: 12,
        }),
        theme: Some("default-dark".to_owned()),
        ..AppearanceValues::default()
    };
    let plan = plan(&base(), &values);
    assert_eq!(plan.font.as_deref(), Some("Noto Sans CJK SC Bold 12"));
    assert_eq!(plan.theme.as_deref(), Some("default-dark"));
    assert!(plan.customize.is_none());
}

#[test]
fn comment_customization_copies_the_selected_theme() {
    let values = AppearanceValues {
        theme: Some("macOS-light".to_owned()),
        comment_color: Some("#112233".to_owned()),
        ..AppearanceValues::default()
    };
    let plan = plan(&base(), &values);
    let customize = plan.customize.expect("改释义颜色要走定制主题");
    assert_eq!(customize.source.as_deref(), Some("macOS-light"));
    assert_eq!(customize.patch.comment_color.as_deref(), Some("#112233"));
    assert_eq!(plan.theme.as_deref(), Some(theme::CUSTOM_THEME_NAME));
}

#[test]
fn comment_change_on_the_custom_theme_patches_in_place() {
    let mut base = base();
    base.theme = theme::CUSTOM_THEME_NAME.to_owned();
    base.comment = CommentStyle {
        color: Some("#112233".to_owned()),
        ..CommentStyle::default()
    };
    // 与现状一致：不动
    let same = AppearanceValues {
        comment_color: Some("#112233".to_owned()),
        ..AppearanceValues::default()
    };
    assert!(plan(&base, &same).is_empty());
    // 同一个颜色的另一种大小写写法也不算修改（主题里是大写、控件回填小写）
    let same_color = AppearanceValues {
        comment_color: Some("#1122AA".to_owned()),
        ..AppearanceValues::default()
    };
    let mut base_upper = base.clone();
    base_upper.comment.color = Some("#1122aa".to_owned());
    assert!(plan(&base_upper, &same_color).is_empty());
    // 改一个字段：就地改写，不再复制
    let changed = AppearanceValues {
        comment_size_factor: Some(100),
        ..AppearanceValues::default()
    };
    let customize = plan(&base, &changed).customize.unwrap();
    assert_eq!(customize.source, None);
    assert_eq!(customize.patch.comment_size_factor, Some(100));
}

#[test]
fn verify_compares_normalized_font_and_exact_theme() {
    let plan = ApplyPlan {
        font: Some("Sans Bold 12".to_owned()),
        theme: Some("default-dark".to_owned()),
        customize: None,
        disable_system_theme: false,
    };
    let mut map = ConfigMap::default();
    map.set("Font", ConfigValue::Text("Sans  Bold 12".to_owned()));
    map.set("Theme", ConfigValue::Text("default-dark".to_owned()));
    assert!(verify(&plan, &map).is_empty());

    map.set("Theme", ConfigValue::Text("default".to_owned()));
    let mismatches = verify(&plan, &map);
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].actual, "default");
}

#[test]
fn font_only_on_a_styled_theme_does_not_customize_it() {
    let mut base = base();
    base.comment.color = Some("#164b73".to_owned());
    base.comment.size_factor = Some(100);
    let plan = plan(
        &base,
        &AppearanceValues {
            font: Some(FontDesc::parse("Sans 12")),
            ..AppearanceValues::default()
        },
    );
    assert!(plan.customize.is_none());
    assert!(plan.theme.is_none());
}

#[test]
fn external_changes_conflict_only_with_edited_fields() {
    let base = base();
    let mut current = base.clone();
    current.font = FontDesc::parse("Sans 14");
    let values = AppearanceValues {
        font: Some(FontDesc::parse("Sans 12")),
        ..AppearanceValues::default()
    };
    assert_eq!(conflicts(&base, &current, &values).len(), 1);
    let values = AppearanceValues {
        font: Some(current.font.clone()),
        ..AppearanceValues::default()
    };
    assert!(conflicts(&base, &current, &values).is_empty());
    assert!(
        conflicts(
            &base,
            &current,
            &AppearanceValues {
                theme: Some("default-dark".to_owned()),
                ..AppearanceValues::default()
            }
        )
        .is_empty()
    );
}
