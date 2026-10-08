//! 候选面板检测：`GetAddonsV2` 看经典面板（classicui）是否启用、有没有别的界面插件可能接管候选显示。

mod addon;
pub use addon::AddonInfo;
mod classic;
pub use classic::ClassicUiPanel;
mod result;
pub use result::PanelProbe;

use gio::glib;

/// fcitx5 的 addon 分类里「用户界面」的编号。
const UI_CATEGORY: i32 = 4;

/// 从 addon 列表算出面板状态。
pub fn probe_addons(addons: &[AddonInfo]) -> PanelProbe {
    let classic_ui = match addons.iter().find(|addon| addon.name == "classicui") {
        None => ClassicUiPanel::Missing,
        Some(addon) if addon.enabled => ClassicUiPanel::Active,
        Some(_) => ClassicUiPanel::Disabled,
    };
    let other_panels = addons
        .iter()
        .filter(|addon| addon.category == UI_CATEGORY && addon.name != "classicui" && addon.enabled)
        .map(|addon| addon.name.clone())
        .collect();
    PanelProbe {
        classic_ui,
        other_panels,
    }
}

/// 解析 `GetAddonsV2` 的回包体（`a(sssibbbasas)`）。
pub fn parse_addons(variant: &glib::Variant) -> Vec<AddonInfo> {
    (0..variant.n_children())
        .filter_map(|index| {
            let entry = variant.child_value(index);
            let name_variant = entry.child_value(0);
            Some(AddonInfo {
                name: name_variant.str()?.to_owned(),
                category: entry.child_value(3).get::<i32>()?,
                // 索引 4 是可配置，5 才是启用，6 是按需加载。
                enabled: entry.child_value(5).get::<bool>()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addon(name: &str, category: i32, enabled: bool, _loaded: bool) -> AddonInfo {
        AddonInfo {
            name: name.to_owned(),
            category,
            enabled,
        }
    }

    #[test]
    fn classic_ui_active_and_other_panels_listed() {
        let probe = probe_addons(&[
            addon("classicui", UI_CATEGORY, true, true),
            addon("kimpanel", UI_CATEGORY, true, true),
            addon("keyboard", 3, true, true),
        ]);
        assert_eq!(probe.classic_ui, ClassicUiPanel::Active);
        assert_eq!(probe.other_panels, vec!["kimpanel".to_owned()]);
    }

    #[test]
    fn disabled_or_missing_classic_ui_is_not_active() {
        let probe = probe_addons(&[addon("classicui", UI_CATEGORY, false, false)]);
        assert_eq!(probe.classic_ui, ClassicUiPanel::Disabled);
        assert_eq!(probe_addons(&[]).classic_ui, ClassicUiPanel::Missing);
    }
}
