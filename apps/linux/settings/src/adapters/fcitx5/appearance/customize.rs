//! ThemeCustomize 的定义与实现。

use crate::adapters::fcitx5::theme::ThemePatch;

/// 定制主题动作：复制源主题（`None` = 当前已是定制主题，就地改写）并打上补丁。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCustomize {
    pub source: Option<String>,

    pub patch: ThemePatch,
}
