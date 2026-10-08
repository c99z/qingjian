//! 外观草稿的脏标记与基准归一：改回现状不算脏、Fcitx5 没跑时草稿保留、应用后清空。

use crate::adapters::fcitx5::{ClassicUiState, FontDesc};
use crate::adapters::fcitx5::{CommentStyle, FontWeight, InstalledTheme};

use super::AppearanceDraft;

fn base() -> ClassicUiState {
    ClassicUiState {
        font: FontDesc::parse("Sans 10"),
        theme: "default".to_owned(),
        vertical: false,
        follow_system_theme: false,
        themes: vec![InstalledTheme {
            name: "default".to_owned(),
            label: "Default".to_owned(),
        }],
        comment: CommentStyle::default(),
    }
}

#[test]
fn fresh_draft_is_not_dirty() {
    let mut draft = AppearanceDraft::new();
    assert!(!draft.is_dirty());
    draft.set_base(Some(base()));
    assert!(!draft.is_dirty());
}

#[test]
fn setting_back_to_the_current_value_is_not_dirty() {
    let mut draft = AppearanceDraft::new();
    draft.set_base(Some(base()));
    draft.set_font(FontDesc {
        family: "Noto Sans CJK SC".to_owned(),
        weight: FontWeight::Bold,
        size: 12,
    });
    assert!(draft.is_dirty());
    draft.set_font(FontDesc::parse("Sans 10"));
    assert!(!draft.is_dirty(), "改回现状自动撤销这条修改");

    draft.set_theme(Some("default-dark".to_owned()));
    assert!(draft.is_dirty());
    draft.set_theme(Some("default".to_owned()));
    assert!(!draft.is_dirty());
    draft.set_theme(None);
    assert!(!draft.is_dirty());
}

#[test]
fn comment_style_compares_against_the_current_theme() {
    let mut draft = AppearanceDraft::new();
    let mut base = base();
    base.comment.color = Some("#164B73".to_owned());
    draft.set_base(Some(base));
    draft.set_comment_color(Some("#164b73".to_owned()));
    assert!(!draft.is_dirty(), "与主题现状一致的颜色不算修改");
    draft.set_comment_color(Some("#112233".to_owned()));
    assert!(draft.is_dirty());
    draft.set_comment_size_factor(Some(100));
    assert!(draft.is_dirty());
}

#[test]
fn edits_without_fcitx5_stay_dirty_until_applied() {
    let mut draft = AppearanceDraft::new();
    // Fcitx5 没跑：基准为空，修改照记、照脏
    draft.set_theme(Some("default".to_owned()));
    assert!(draft.is_dirty());
    draft.clear();
    assert!(!draft.is_dirty());
}

#[test]
fn probing_does_not_replace_the_baseline_of_edited_fields() {
    let mut draft = AppearanceDraft::new();
    draft.set_base(Some(base()));
    draft.set_theme(Some("default-dark".to_owned()));
    assert!(draft.is_dirty());
    // 外部已把主题换成了草稿想要的值：与新现状一致，不再算脏
    let mut new_base = base();
    new_base.theme = "default-dark".to_owned();
    draft.set_base(Some(new_base));
    assert!(draft.is_dirty());
    assert_eq!(draft.base().unwrap().theme, "default");
}
