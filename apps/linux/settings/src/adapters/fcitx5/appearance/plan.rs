//! ApplyPlan 的定义与实现。

use crate::adapters::fcitx5::appearance::ThemeCustomize;

/// 一次应用要做的事。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyPlan {
    /// 写入 classicui `Font` 的新值（Pango 字体描述串）。
    pub font: Option<String>,

    /// 写入 classicui `Theme` 的新值；定制主题时指向 [`crate::adapters::fcitx5::theme::CUSTOM_THEME_NAME`]。
    pub theme: Option<String>,

    /// 需要复制 / 改写定制主题。
    pub customize: Option<ThemeCustomize>,

    /// 显式主题选择固定外观，停止被系统 DarkTheme 覆盖。
    pub disable_system_theme: bool,
}

impl ApplyPlan {
    pub fn is_empty(&self) -> bool {
        self.font.is_none()
            && self.theme.is_none()
            && self.customize.is_none()
            && !self.disable_system_theme
    }
}
