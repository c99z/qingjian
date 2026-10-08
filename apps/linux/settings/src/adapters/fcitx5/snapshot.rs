//! 经典面板当前配置的快照：外观草稿的基准（「改回现状就不算脏」以它为准）。

use super::font::FontDesc;
use super::theme::{CommentStyle, InstalledTheme};

/// classicui 的当前配置，由 `GetConfig` + 主题目录读出来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicUiState {
    /// `Font` 解析后的字体。
    pub font: FontDesc,

    /// `Theme`：当前主题名。
    pub theme: String,

    /// `Vertical Candidate List`（仅展示用；候选排布由青简 `general.layout` 经插件提示给面板）。
    pub vertical: bool,

    /// classicui 是否按系统深浅色切换到 DarkTheme。
    pub follow_system_theme: bool,

    /// 已安装主题（配置描述里 Theme 字段的枚举值与显示名）。
    pub themes: Vec<InstalledTheme>,

    /// 当前主题的释义样式（读它的 `theme.conf`）。
    pub comment: CommentStyle,
}
