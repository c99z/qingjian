//! 外观应用：检查外部冲突，准备可回滚主题，写入并核对实时配置，失败恢复原值。

use crate::adapters::fcitx5::appearance::{self, AppearanceValues, FieldMismatch};
use crate::adapters::fcitx5::client::Fcitx5Client;
use crate::adapters::fcitx5::error::Fcitx5Error;
use crate::adapters::fcitx5::probe::ClassicUiPanel;
use crate::adapters::fcitx5::snapshot::ClassicUiState;
use crate::adapters::fcitx5::theme::{CommentStyle, ThemePatch, ThemeTransaction};

pub fn apply(
    client: &Fcitx5Client,
    base: &ClassicUiState,
    values: &AppearanceValues,
) -> Result<(), Fcitx5Error> {
    if client.probe_panels()?.classic_ui != ClassicUiPanel::Active {
        return Err(Fcitx5Error::PanelUnavailable);
    }
    appearance::preflight(client, base, values)?;
    let current = client.classic_ui()?;
    let conflicts = appearance::conflicts(base, &current, values);
    if !conflicts.is_empty() {
        return Err(Fcitx5Error::Conflict(conflicts));
    }
    let plan = appearance::plan(&current, values);
    if plan.is_empty() {
        return Ok(());
    }
    let original = client.classic_ui_map()?;
    let transaction = plan
        .customize
        .as_ref()
        .map(|custom| ThemeTransaction::prepare(custom.source.as_deref(), &custom.patch))
        .transpose()?;
    let mut map = original.clone();
    appearance::merge_into(&plan, &mut map);
    let result = (|| {
        client.set_classic_ui(&map)?;
        client.reload_addon("classicui")?;
        let readback = client.classic_ui_map()?;
        let mut mismatches = appearance::verify(&plan, &readback);
        if let Some(custom) = &plan.customize {
            let style = client.theme_style(readback.text("Theme").unwrap_or("default"))?;
            mismatches.extend(verify_style(&custom.patch, &style));
        }
        if mismatches.is_empty() {
            Ok(())
        } else {
            Err(Fcitx5Error::Verify(mismatches))
        }
    })();
    match result {
        Ok(()) => {
            if let Some(transaction) = transaction {
                transaction.commit();
            }
            Ok(())
        }
        Err(error) => {
            let theme_restore = transaction.map(ThemeTransaction::rollback).transpose();
            let config_restore = client
                .set_classic_ui(&original)
                .and_then(|()| client.reload_addon("classicui"));
            theme_restore?;
            config_restore?;
            Err(error)
        }
    }
}

fn verify_style(patch: &ThemePatch, style: &CommentStyle) -> Vec<FieldMismatch> {
    let mut out = Vec::new();
    for (label, want, actual) in [
        ("中文颜色", &patch.normal_color, &style.normal_color),
        (
            "高亮中文颜色",
            &patch.highlight_color_text,
            &style.highlight_color_text,
        ),
        ("释义颜色", &patch.comment_color, &style.color),
        (
            "高亮释义颜色",
            &patch.highlight_comment_color,
            &style.highlight_color,
        ),
    ] {
        if let Some(want) = want
            && actual
                .as_ref()
                .is_none_or(|v| !v.eq_ignore_ascii_case(want))
        {
            out.push(FieldMismatch {
                label,
                expected: want.clone(),
                actual: actual.clone().unwrap_or_default(),
            });
        }
    }
    if let Some(want) = patch.comment_size_factor
        && Some(want) != style.size_factor
    {
        out.push(FieldMismatch {
            label: "释义字号",
            expected: want.to_string(),
            actual: style.size_factor.unwrap_or(75).to_string(),
        });
    }
    out
}
