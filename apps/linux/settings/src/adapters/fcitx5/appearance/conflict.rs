//! 外观字段三路比较：只阻止同时修改了同一字段且目标不一致的保存。

use crate::adapters::fcitx5::appearance::{AppearanceValues, FieldMismatch};
use crate::adapters::fcitx5::snapshot::ClassicUiState;

pub fn conflicts(
    base: &ClassicUiState,
    current: &ClassicUiState,
    values: &AppearanceValues,
) -> Vec<FieldMismatch> {
    let mut out = Vec::new();
    if let Some(want) = &values.font
        && base.font != current.font
        && *want != current.font
    {
        out.push(FieldMismatch {
            label: "字体",
            expected: want.compose(),
            actual: current.font.compose(),
        });
    }
    let custom = values.normal_color.is_some()
        || values.highlight_color_text.is_some()
        || values.comment_color.is_some()
        || values.highlight_comment_color.is_some()
        || values.comment_size_factor.is_some();
    if (values.theme.is_some() || custom)
        && base.theme != current.theme
        && values.theme.as_ref() != Some(&current.theme)
    {
        out.push(FieldMismatch {
            label: "主题",
            expected: values.theme.clone().unwrap_or_else(|| base.theme.clone()),
            actual: current.theme.clone(),
        });
    }
    for (label, original, actual, want) in [
        (
            "中文颜色",
            &base.comment.normal_color,
            &current.comment.normal_color,
            &values.normal_color,
        ),
        (
            "高亮中文颜色",
            &base.comment.highlight_color_text,
            &current.comment.highlight_color_text,
            &values.highlight_color_text,
        ),
        (
            "释义颜色",
            &base.comment.color,
            &current.comment.color,
            &values.comment_color,
        ),
        (
            "高亮释义颜色",
            &base.comment.highlight_color,
            &current.comment.highlight_color,
            &values.highlight_comment_color,
        ),
    ] {
        let equal = |a: &Option<String>, b: &Option<String>| match (a, b) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            (None, None) => true,
            _ => false,
        };
        if want.is_some() && !equal(original, actual) && !equal(want, actual) {
            out.push(FieldMismatch {
                label,
                expected: want.clone().unwrap_or_default(),
                actual: actual.clone().unwrap_or_default(),
            });
        }
    }
    if let Some(want) = values.comment_size_factor
        && base.comment.size_factor != current.comment.size_factor
        && Some(want) != current.comment.size_factor
    {
        out.push(FieldMismatch {
            label: "释义字号",
            expected: want.to_string(),
            actual: current.comment.size_factor.unwrap_or(75).to_string(),
        });
    }
    out
}
