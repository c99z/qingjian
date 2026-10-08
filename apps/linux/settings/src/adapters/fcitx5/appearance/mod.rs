//! 外观草稿的取值与「草稿 → 写入计划」的纯逻辑：哪几个 classicui 字段要改、要不要复制定制主题。
//! 不碰 D-Bus 与文件系统，全部可单测。

mod conflict;
pub use conflict::conflicts;
mod values;
pub use values::AppearanceValues;
mod customize;
pub use customize::ThemeCustomize;
mod plan;
pub use plan::ApplyPlan;
mod mismatch;
pub use mismatch::FieldMismatch;

use crate::adapters::fcitx5::font::{FontDesc, fonts_match};
use crate::adapters::fcitx5::snapshot::ClassicUiState;
use crate::adapters::fcitx5::theme::{self, ThemePatch};
use crate::adapters::fcitx5::value::{ConfigMap, ConfigValue};

/// 由草稿算出写入计划：与当前值一致的字段不写；释义样式有定制时复制 / 改写定制主题并把
/// classicui `Theme` 指过去（选中已有主题不动原主题，见 [`theme::ThemeTransaction`]）。
pub fn plan(base: &ClassicUiState, values: &AppearanceValues) -> ApplyPlan {
    let font = values
        .font
        .as_ref()
        .filter(|font| **font != base.font)
        .map(FontDesc::compose);

    let patch = ThemePatch {
        normal_color: values.normal_color.clone(),
        highlight_color_text: values.highlight_color_text.clone(),
        comment_color: values.comment_color.clone(),
        highlight_comment_color: values.highlight_comment_color.clone(),
        comment_size_factor: values.comment_size_factor,
    };
    let color_changed = |a: Option<&String>, b: Option<&String>| {
        a.is_some_and(|a| b.is_none_or(|b| !a.eq_ignore_ascii_case(b)))
    };
    let comment_changed = color_changed(values.comment_color.as_ref(), base.comment.color.as_ref())
        || color_changed(
            values.highlight_comment_color.as_ref(),
            base.comment.highlight_color.as_ref(),
        )
        || color_changed(
            values.normal_color.as_ref(),
            base.comment.normal_color.as_ref(),
        )
        || color_changed(
            values.highlight_color_text.as_ref(),
            base.comment.highlight_color_text.as_ref(),
        )
        || values
            .comment_size_factor
            .is_some_and(|v| Some(v) != base.comment.size_factor);
    let customize = if comment_changed {
        let source = values.theme.clone().unwrap_or_else(|| base.theme.clone());
        // 当前已经是定制主题：就地改写，不再复制
        let source = (source != theme::CUSTOM_THEME_NAME).then_some(source);
        Some(ThemeCustomize { source, patch })
    } else {
        None
    };

    let theme = if customize.is_some() {
        Some(theme::CUSTOM_THEME_NAME.to_owned())
    } else {
        values.theme.clone().filter(|name| *name != base.theme)
    };

    let disable_system_theme =
        (values.theme.is_some() || customize.is_some()) && base.follow_system_theme;
    ApplyPlan {
        disable_system_theme,
        font,
        theme,
        customize,
    }
}

/// 写完后拿读回的配置核对：字体按归一化比较（`Sans Bold 10` 与 `Sans  Bold 10` 算同一个），
/// 主题按原文比较。
pub fn verify(plan: &ApplyPlan, readback: &ConfigMap) -> Vec<FieldMismatch> {
    let mut mismatches = Vec::new();
    if let Some(font) = &plan.font {
        let actual = readback.text("Font").unwrap_or("").to_owned();
        if !fonts_match(font, &actual) {
            mismatches.push(FieldMismatch {
                label: "字体",
                expected: font.clone(),
                actual,
            });
        }
    }
    if let Some(theme) = &plan.theme {
        let actual = readback.text("Theme").unwrap_or("").to_owned();
        if *theme != actual {
            mismatches.push(FieldMismatch {
                label: "主题",
                expected: theme.clone(),
                actual,
            });
        }
    }
    if plan.disable_system_theme && readback.boolean("UseDarkTheme") != Some(false) {
        mismatches.push(FieldMismatch {
            label: "固定主题",
            expected: "False".to_owned(),
            actual: readback.text("UseDarkTheme").unwrap_or("").to_owned(),
        });
    }
    mismatches
}

/// 把计划写进一份刚读出来的完整配置（`SetConfig` 发全量，与 fcitx5-configtool 的做法一致）。
pub fn merge_into(plan: &ApplyPlan, map: &mut ConfigMap) {
    if plan.disable_system_theme {
        map.set("UseDarkTheme", ConfigValue::Text("False".to_owned()));
    }
    if let Some(font) = &plan.font {
        map.set("Font", ConfigValue::Text(font.clone()));
    }
    if let Some(theme) = &plan.theme {
        map.set("Theme", ConfigValue::Text(theme.clone()));
    }
}

#[cfg(test)]
mod tests;

/// 写任何文件之前先检查外观草稿；离线由上层单独保留外观，其他配置仍可保存。
pub fn preflight(
    client: &crate::adapters::fcitx5::Fcitx5Client,
    base: &ClassicUiState,
    values: &AppearanceValues,
) -> Result<(), crate::adapters::fcitx5::Fcitx5Error> {
    use crate::adapters::fcitx5::Fcitx5Error;
    let current = client.classic_ui()?;
    let conflicts = conflicts(base, &current, values);
    if !conflicts.is_empty() {
        return Err(Fcitx5Error::Conflict(conflicts));
    }
    if let Some(font) = &values.font
        && (font.family.trim().is_empty()
            || font.family.contains(['\n', '\r'])
            || !(6..=32).contains(&font.size))
    {
        return Err(Fcitx5Error::Invalid(
            "字体名不能为空，字号应在 6 至 32 之间".to_owned(),
        ));
    }
    for color in [
        &values.normal_color,
        &values.highlight_color_text,
        &values.comment_color,
        &values.highlight_comment_color,
    ]
    .into_iter()
    .flatten()
    {
        if crate::adapters::fcitx5::color::parse_hex(color).is_none() {
            return Err(Fcitx5Error::Invalid(
                "颜色格式应为 #RRGGBB 或 #RRGGBBAA".to_owned(),
            ));
        }
    }
    if let Some(value) = values.comment_size_factor
        && !(1..=400).contains(&value)
    {
        return Err(Fcitx5Error::Invalid(
            "释义字号比例应在 1 至 400 之间".to_owned(),
        ));
    }
    if let Some(name) = &values.theme
        && theme::find_theme_dir(name).is_none()
    {
        return Err(Fcitx5Error::ThemeNotFound(name.clone()));
    }
    Ok(())
}
